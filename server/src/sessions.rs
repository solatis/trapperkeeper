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
        // Names are stored lowercase (ADR-0020). The lookup skips
        // `scope::tenant`, so the route also reaches `$tk` and the
        // superadmin's own account.
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
    // The audit row is filed under the user's tenant, so its admins see who
    // minted a session for whom.
    caller.identity.set_scope(&user.tenant_id);
    // No cookie is set, so the superadmin's own browser session stays.
    Json(login_body(
        &state,
        &user,
        // A password change passes `false` here. A mint passes the flag as
        // stored, because the flag restricts no route.
        user.password_must_change,
        SystemTime::now(),
    ))
    .into_response()
}
