//! The superadmin's routes for tenants: listing them and creating one with
//! its first admin.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::accounts::{self, NewAccount};
use crate::auth::login::database_error;
use crate::auth::role::{self, AtLeast};
use crate::http::AppState;
use crate::names;
use crate::store::tenants::{self, ADMIN_ROLE, NewTenant};
use crate::store::users;

/// A tenant as the list shows it.
#[derive(Serialize, ToSchema)]
pub struct TenantView {
    name: String,
    title: String,
}

/// Lists the live tenants, without the system tenant `$tk`. Needs superadmin.
#[utoipa::path(
    get,
    path = "/tenants",
    security(("bearer" = [])),
    responses(
        (status = OK, description = "the tenants, by name", body = Vec<TenantView>),
        (status = UNAUTHORIZED, description = "no valid access token", body = String),
        (status = FORBIDDEN, description = "the caller is not the superadmin", body = String)
    )
)]
pub async fn list(_caller: AtLeast<role::Superadmin>, State(state): State<AppState>) -> Response {
    let listed = async {
        let mut tx = state.store.begin().await?;
        tenants::list(&mut tx).await
    }
    .await;
    match listed {
        Ok(listed) => Json(
            listed
                .into_iter()
                .map(|t| TenantView {
                    name: t.name,
                    title: t.title,
                })
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(error) => database_error(&error),
    }
}

/// A tenant to create and the email of its first admin.
#[derive(Deserialize, ToSchema)]
pub struct NewTenantRequest {
    name: String,
    title: String,
    /// The email address the first admin logs in with.
    admin: String,
}

/// What a created tenant returns: the tenant's stored name and title, the
/// admin's name and the admin's generated password. No other response returns
/// that password, because the store keeps only its hash and the audit trail
/// stores no body.
#[derive(Serialize, ToSchema)]
pub struct CreatedTenant {
    name: String,
    title: String,
    admin: String,
    admin_password: String,
}

/// Creates a tenant with the roles admin, operator and observer, and its first
/// admin with a generated password that must be changed. Needs superadmin.
/// The route lowercases both names and trims the title before it stores and
/// returns them.
#[utoipa::path(
    post,
    path = "/tenants",
    security(("bearer" = [])),
    request_body = NewTenantRequest,
    responses(
        (status = CREATED, description = "the tenant and its admin's one-time password", body = CreatedTenant),
        (status = BAD_REQUEST, description = "a name or the title breaks its rule", body = String),
        (status = UNAUTHORIZED, description = "no valid access token", body = String),
        (status = FORBIDDEN, description = "the caller is not the superadmin", body = String),
        (status = CONFLICT, description = "a tenant has the name", body = String)
    )
)]
pub async fn create(
    _caller: AtLeast<role::Superadmin>,
    State(state): State<AppState>,
    Json(request): Json<NewTenantRequest>,
) -> Response {
    // The handler checks the tenant name, the admin's name and the title,
    // then creates the tenant and its first admin in one transaction, and
    // answers 201 with the admin's generated password.
    //
    // 1. We check the tenant name and the admin's email with the rules in
    //    `names`, and the trimmed title for 1 to 255 characters. The trimmed
    //    title is also what the store keeps and the response returns. A
    //    refusal answers 400 with the words of the rule that refused. Every
    //    check runs before the transaction opens, so a refused request writes
    //    nothing.
    // 2. We open a transaction and create the tenant with the roles admin,
    //    operator and observer, which gives the tenant's id.
    // 3. We create the first admin in the new tenant with the role admin,
    //    through `accounts::create`, which generates the admin's password,
    //    sets the must-change flag and returns the password. Every user is
    //    created through `accounts::create`, so every user gets a generated
    //    password with the must-change flag set.
    // 4. We commit, then answer 201 with the stored name and title, the
    //    admin's name and the password, 409 when a live tenant has the name,
    //    and 500 on a database error.

    // 1. check the names and the title
    let title = request.title.trim();
    // The checks run in the order tenant name, admin name, title, and the
    // first that fails supplies the text of the 400.
    let checked = names::tenant_name(&request.name).and_then(|name| {
        let admin = names::user_name(&request.admin)?;
        // 255 is the width of the `tenant_title` column on MariaDB, a
        // utf8mb4 VARCHAR(255) that counts characters, so the check counts
        // characters and not bytes.
        if (1..=255).contains(&title.chars().count()) {
            Ok((name, admin))
        } else {
            Err(names::Refused("the title must be 1 to 255 characters"))
        }
    });
    let (name, admin) = match checked {
        Ok(checked) => checked,
        Err(refused) => return (StatusCode::BAD_REQUEST, refused.0).into_response(),
    };

    let created = async {
        // 2. create the tenant and its roles
        let mut tx = state.store.begin().await?;
        let tenant_id = tenants::create(&mut tx, NewTenant { name: &name, title }).await?;
        // 3. create the admin
        let password = accounts::create(
            &mut tx,
            NewAccount {
                tenant_id: &tenant_id,
                name: &admin,
                role_name: ADMIN_ROLE,
            },
        )
        .await
        // The closure turns the user error into a tenant error, so every `?`
        // in this block yields `tenants::CreateError`, which the match below
        // answers.
        .map_err(|error| match error {
            users::CreateError::Database(error) => tenants::CreateError::Database(error),
            // The tenant was created in this transaction, so it has no users.
            users::CreateError::NameTaken => unreachable!("a new tenant has no users"),
        })?;
        // 4. commit and answer
        tx.commit().await?;
        Ok(password)
    }
    .await;
    match created {
        Ok(admin_password) => (
            StatusCode::CREATED,
            Json(CreatedTenant {
                name,
                title: title.to_owned(),
                admin,
                admin_password,
            }),
        )
            .into_response(),
        Err(tenants::CreateError::NameTaken) => {
            (StatusCode::CONFLICT, "a tenant with this name exists").into_response()
        }
        Err(tenants::CreateError::Database(error)) => database_error(&error),
    }
}
