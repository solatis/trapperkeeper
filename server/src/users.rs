//! The admin's routes for the users of a tenant, under
//! `/tenants/{tenant}/users`.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::accounts::{self, NewAccount};
use crate::auth::login::database_error;
use crate::auth::role::{self, AtLeast};
use crate::http::AppState;
use crate::names;
use crate::scope;
use crate::store::tenants;
use crate::store::users;
use crate::superadmin;

/// Why a user route changed nothing. It converts into the response the
/// caller gets.
enum Refused {
    Scope(scope::Refused),
    /// The request's content breaks a rule: 400 with the rule.
    BadRequest(&'static str),
    /// A live user of the tenant has the name: 409.
    NameTaken,
    Store(Box<sqlx::Error>),
}

impl From<scope::Refused> for Refused {
    fn from(refused: scope::Refused) -> Self {
        Self::Scope(refused)
    }
}

impl From<sqlx::Error> for Refused {
    fn from(error: sqlx::Error) -> Self {
        Self::Store(Box::new(error))
    }
}

impl From<users::CreateError> for Refused {
    fn from(error: users::CreateError) -> Self {
        match error {
            users::CreateError::NameTaken => Self::NameTaken,
            users::CreateError::Database(error) => Self::Store(Box::new(error)),
        }
    }
}

impl IntoResponse for Refused {
    fn into_response(self) -> Response {
        match self {
            Self::Scope(refused) => refused.into_response(),
            Self::BadRequest(rule) => (StatusCode::BAD_REQUEST, rule).into_response(),
            Self::NameTaken => {
                (StatusCode::CONFLICT, "a user with this name exists").into_response()
            }
            Self::Store(error) => database_error(&error),
        }
    }
}

/// A user as the list shows it.
#[derive(Serialize, ToSchema)]
pub struct UserView {
    tenant_id: String,
    name: String,
    role: String,
    password_must_change: bool,
}

/// Lists the live users of the tenant, by name. Needs admin of that tenant,
/// or superadmin.
#[utoipa::path(
    get,
    path = "/tenants/{tenant}/users",
    params(("tenant" = String, Path, description = "the tenant's name")),
    security(("bearer" = [])),
    responses(
        (status = OK, description = "the users, by name", body = Vec<UserView>),
        (status = UNAUTHORIZED, description = "no valid access token", body = String),
        (status = FORBIDDEN, description = "the caller is below admin, or of another tenant", body = String),
        (status = NOT_FOUND, description = "no such tenant", body = String)
    )
)]
pub async fn list(
    caller: AtLeast<role::Admin>,
    State(state): State<AppState>,
    Path(tenant): Path<String>,
) -> Response {
    let listed = async {
        let mut tx = state.store.begin().await?;
        let scope = scope::resolve(&mut tx, &caller.user, &tenant).await?;
        Ok::<_, Refused>(users::list(&mut tx, &scope.tenant.id).await?)
    }
    .await;
    match listed {
        Ok(listed) => Json(listed.into_iter().map(view).collect::<Vec<_>>()).into_response(),
        Err(refused) => refused.into_response(),
    }
}

/// `user` as the list shows it.
fn view(user: users::User) -> UserView {
    UserView {
        tenant_id: user.tenant_id,
        name: user.name,
        role: user.role_name,
        password_must_change: user.password_must_change,
    }
}

/// Who to create: an email address and one of the tenant's roles.
#[derive(Deserialize, ToSchema)]
pub struct NewUserRequest {
    name: String,
    role: String,
}

/// What a create or a reset hands back: the generated password, shown once.
#[derive(Serialize, ToSchema)]
pub struct CreatedUser {
    tenant_id: String,
    name: String,
    role: String,
    password: String,
}

/// Creates a user of the tenant with a generated password that must be
/// changed. Needs admin of that tenant, or superadmin. 400 for a bad name, a
/// role the tenant does not hold, or `superadmin`; 409 when a live user has
/// the name.
#[utoipa::path(
    post,
    path = "/tenants/{tenant}/users",
    params(("tenant" = String, Path, description = "the tenant's name")),
    security(("bearer" = [])),
    request_body = NewUserRequest,
    responses(
        (status = CREATED, description = "the user and its one-time password", body = CreatedUser),
        (status = BAD_REQUEST, description = "the name breaks its rule, or the tenant does not hold the role", body = String),
        (status = UNAUTHORIZED, description = "no valid access token", body = String),
        (status = FORBIDDEN, description = "the caller is below admin, or of another tenant", body = String),
        (status = NOT_FOUND, description = "no such tenant", body = String),
        (status = CONFLICT, description = "a user of the tenant has the name", body = String)
    )
)]
pub async fn create(
    caller: AtLeast<role::Admin>,
    State(state): State<AppState>,
    Path(tenant): Path<String>,
    Json(request): Json<NewUserRequest>,
) -> Response {
    // 1. check the name's shape and refuse the superadmin role
    // 2. open the transaction, resolve the scope, check the tenant holds the role
    // 3. create the user through accounts::create
    // 4. commit and return the password

    // 1. `superadmin` is refused by name before any read, because `$tk` holds
    //    it alone and a second superadmin was rejected (doc/adr/0028).
    let name = match names::user_name(&request.name) {
        Ok(name) => name,
        Err(refused) => return Refused::BadRequest(refused.0).into_response(),
    };
    if request.role == superadmin::ROLE {
        return Refused::BadRequest("no route gives the role superadmin").into_response();
    }

    let created = async {
        let mut tx = state.store.begin().await?;
        let scope = scope::resolve(&mut tx, &caller.user, &tenant).await?;
        // 2. The role is checked before the insert, because a role the tenant
        //    does not hold would otherwise fail the insert's NOT NULL role id,
        //    which is a 500.
        if !tenants::role_exists(&mut tx, &scope.tenant.id, &request.role).await? {
            return Err(Refused::BadRequest("the tenant does not hold this role"));
        }
        // 3. Through accounts::create, so the user gets a generated password
        //    that must be changed.
        let password = accounts::create(
            &mut tx,
            NewAccount {
                tenant_id: &scope.tenant.id,
                name: &name,
                role_name: &request.role,
            },
        )
        .await?;
        tx.commit().await?;
        Ok((scope.tenant.id, password))
    }
    .await;

    // 4. The password is in this response and nowhere else (doc/adr/0028).
    match created {
        Ok((tenant_id, password)) => (
            StatusCode::CREATED,
            Json(CreatedUser {
                tenant_id,
                name,
                role: request.role,
                password,
            }),
        )
            .into_response(),
        Err(refused) => refused.into_response(),
    }
}
