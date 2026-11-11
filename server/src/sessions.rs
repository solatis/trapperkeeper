//! The superadmin's route that mints a session for a user: the body a login
//! hands that user, without the cookies. The tenant and the user are path
//! parameters rather than a body, so the audit trail, which stores path
//! parameters and never a body, records who minted a session for whom.

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
/// tenant, as a login body without cookies. Needs superadmin. 404 when the
/// tenant or the user is not live.
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
    // Both names lowercased, as login does (ADR-0020). Nothing refuses
    // the superadmin's own account, because that mint is the same as a
    // refresh.
    let found = async {
        let mut tx = state.store.begin().await?;
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
    // Noted so the audit row is filed under the user's tenant, whose admins
    // then see who minted a session for whom.
    caller.identity.set_scope(&user.tenant_id);
    // The body alone, no cookie, so the superadmin's own browser session
    // stays and the client decides what to do with the tokens. The
    // must-change flag passes through as it is, because it restricts nothing
    // and the client shows it.
    Json(login_body(
        &state,
        &user,
        user.password_must_change,
        SystemTime::now(),
    ))
    .into_response()
}
