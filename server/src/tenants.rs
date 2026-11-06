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

/// Lists the live tenants. Needs superadmin.
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

/// What a created tenant hands back: the admin's generated password, shown
/// once.
#[derive(Serialize, ToSchema)]
pub struct CreatedTenant {
    name: String,
    title: String,
    admin: String,
    admin_password: String,
}

/// Creates a tenant with the roles admin, operator and observer, and its first
/// admin with a generated password that must be changed. Needs superadmin.
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
    // 1. check the tenant name, the title and the admin's name
    // 2. create the tenant and its roles
    // 3. create the admin
    // 4. commit and return the password

    // 1. Every check runs before the transaction opens, so a refused request
    //    writes nothing. 255 characters is the title column's width on
    //    MariaDB.
    let title = request.title.trim();
    let checked = names::tenant_name(&request.name).and_then(|name| {
        let admin = names::user_name(&request.admin)?;
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
        let mut tx = state.store.begin().await?;
        // 2.
        let tenant_id = tenants::create(&mut tx, NewTenant { name: &name, title }).await?;
        // 3. Through accounts::create, so the admin gets a generated password
        //    that must be changed.
        let password = accounts::create(
            &mut tx,
            NewAccount {
                tenant_id: &tenant_id,
                name: &admin,
                role_name: ADMIN_ROLE,
            },
        )
        .await
        .map_err(|error| match error {
            users::CreateError::Database(error) => tenants::CreateError::Database(error),
            // The tenant was created in this transaction, so it has no users.
            users::CreateError::NameTaken => unreachable!("a new tenant has no users"),
        })?;
        tx.commit().await?;
        Ok(password)
    }
    .await;

    // 4. The password appears in this response and nowhere else.
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
