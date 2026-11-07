//! The admin's routes for the users of a tenant, under
//! `/tenants/{tenant}/users`.

use axum::Json;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use utoipa::ToSchema;

use crate::auth::role::{self, AtLeast};
use crate::http::AppState;
use crate::scope;
use crate::store::users;

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
        Ok::<_, scope::Refused>(users::list(&mut tx, &scope.tenant.id).await?)
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
