//! The superadmin's route that mints a session for a user. The tenant and the
//! user are path parameters, because every tenant-scoped route follows the
//! model `/tenants/{tenant}/users/{name}` (ADR-0030). The audit trail, which
//! stores path parameters and never a body, therefore records who minted a
//! session for whom.

use std::time::SystemTime;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::auth::login::{Login, database_error, login_body};
use crate::auth::role::{self, AtLeast};
use crate::http::AppState;
use crate::store::users;

/// Mints an access token and a refresh token for the named user of the named
/// tenant, as a login body without cookies. Needs superadmin. Answers 404
/// when the tenant or the user does not exist or was deleted.
#[utoipa::path(
    post,
    path = "/tenants/{tenant}/users/{name}/session",
    params(
        ("tenant" = String, Path, description = "the tenant's name"),
        ("name" = String, Path, description = "the user's email address")
    ),
    security(("bearer" = [])),
    responses(
        (status = OK, description = "the user's tokens", body = Login),
        (status = UNAUTHORIZED, description = "no valid access token", body = String),
        (status = FORBIDDEN, description = "the caller is not the superadmin", body = String),
        (status = NOT_FOUND, description = "no such tenant or user", body = String)
    )
)]
pub async fn mint(
    caller: AtLeast<role::Superadmin>,
    State(state): State<AppState>,
    Path((tenant, name)): Path<(String, String)>,
) -> Response {
    let found = async {
        let mut tx = state.store.begin().await?;
        // The handler lowercases both names before the lookup, as login does,
        // because names are stored lowercase (ADR-0020). It looks the user up
        // with `users::find_in` and not through `scope::tenant`, so the route
        // reaches the system tenant `$tk` and lets the superadmin mint a
        // session for its own account, which issues a new refresh token as a
        // login does.
        users::find_in(&mut tx, &tenant.to_lowercase(), &name.to_lowercase()).await
    }
    .await;
    let user = match found {
        Ok(Some(user)) => user,
        // 404 rather than login's uniform 401, because the caller is already
        // the superadmin and nothing is hidden from it.
        Ok(None) => return (StatusCode::NOT_FOUND, "no such tenant or user").into_response(),
        Err(error) => return database_error(&error),
    };
    // The handler records the user's tenant as the request's scope, so the
    // audit middleware files the row under that tenant, whose admins see the
    // superadmin as the caller and both names in the path parameters.
    caller.identity.set_scope(&user.tenant_id);
    // The handler returns the body and sets no cookie, so the superadmin's
    // own browser session stays and the client decides how to use the
    // tokens.
    Json(login_body(
        &state,
        &user,
        // The must-change flag passes through as stored, because it
        // restricts no route and the client shows it.
        user.password_must_change,
        SystemTime::now(),
    ))
    .into_response()
}
