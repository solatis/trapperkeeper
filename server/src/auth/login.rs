//! The login route, and what a logged-in caller can read about itself.

use std::time::SystemTime;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::{Deserialize, Serialize};
use tokio::sync::OnceCell;
use utoipa::ToSchema;

use crate::auth::caller::Caller;
use crate::auth::password;
use crate::auth::token::Kind;
use crate::http::AppState;
use crate::store::DEFAULT_TENANT_ID;
use crate::store::users;

/// A name and a password.
#[derive(Deserialize, ToSchema)]
pub struct Credentials {
    name: String,
    password: String,
}

/// The cookie that carries the access token for the browser.
pub const ACCESS_COOKIE: &str = "tk_access";

/// What a login hands out. The access token also comes back in the
/// `tk_access` cookie.
#[derive(Serialize, ToSchema)]
pub struct Login {
    /// The access token, sent back as `Authorization: Bearer`.
    access_token: String,
    /// Always `Bearer`.
    token_type: &'static str,
    /// How many seconds the access token lives.
    expires_in: u64,
    /// Whether the UI should ask for a new password now.
    password_must_change: bool,
}

/// A hash no password matches, so a login with an unknown name costs a hash
/// too.
static DUMMY_HASH: OnceCell<String> = OnceCell::const_new();

/// Logs a user of the default tenant in with a name and a password. Returns
/// an access token, in the body and in the `tk_access` cookie.
#[utoipa::path(
    post,
    path = "/login",
    request_body = Credentials,
    responses(
        (status = OK, description = "the access token", body = Login),
        (status = UNAUTHORIZED, description = "the name or the password is wrong", body = String)
    )
)]
pub async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(credentials): Json<Credentials>,
) -> Response {
    // Names are stored lowercase (doc/adr/0020).
    let name = credentials.name.to_lowercase();
    // The product runs one tenant, so login takes none (doc/adr/0003).
    let found = async {
        let mut tx = state.store.begin().await?;
        users::find(&mut tx, DEFAULT_TENANT_ID, &name).await
    }
    .await;
    let user = match found {
        Ok(user) => user,
        Err(error) => return database_error(&error),
    };
    // An unknown name is checked against the dummy hash, so the response time
    // does not tell whether the name exists.
    let hash = match &user {
        Some(user) => user.password_hash.clone(),
        None => DUMMY_HASH
            .get_or_init(|| password::hash(String::new()))
            .await
            .clone(),
    };
    let matches = password::verify(credentials.password, hash).await;
    let Some(user) = user.filter(|_| matches) else {
        return unauthorized();
    };
    let access_token =
        state
            .keyring
            .issue(Kind::Access, &user.id, &user.tenant_id, SystemTime::now());
    // HttpOnly so page scripts cannot read it, SameSite=Strict so cross-site
    // requests do not carry it, Secure because TLS is terminated directly or
    // by a proxy and browsers treat localhost as secure, and Max-Age the
    // token's lifetime.
    let cookie = Cookie::build((ACCESS_COOKIE, access_token.clone()))
        .http_only(true)
        .same_site(SameSite::Strict)
        .secure(true)
        .path("/")
        .max_age(cookie::time::Duration::seconds(
            i64::try_from(Kind::Access.lifetime().as_secs()).expect("an hour fits"),
        ))
        .build();
    (
        jar.add(cookie),
        Json(Login {
            access_token,
            token_type: "Bearer",
            expires_in: Kind::Access.lifetime().as_secs(),
            password_must_change: user.password_must_change,
        }),
    )
        .into_response()
}

/// What a caller can read about itself.
#[derive(Serialize, ToSchema)]
pub struct Me {
    name: String,
    role: String,
    password_must_change: bool,
}

/// The caller's name, role, and whether the password must be changed.
#[utoipa::path(
    get,
    path = "/me",
    security(("bearer" = [])),
    responses(
        (status = OK, description = "the caller", body = Me),
        (status = UNAUTHORIZED, description = "no valid access token", body = String)
    )
)]
pub async fn me(caller: Caller) -> Json<Me> {
    Json(Me {
        name: caller.user.name,
        role: caller.user.role_name,
        password_must_change: caller.user.password_must_change,
    })
}

/// The one refusal every authentication failure gets, so a response does not
/// say which name exists or why a check failed.
pub fn unauthorized() -> Response {
    (StatusCode::UNAUTHORIZED, "unauthorized").into_response()
}

/// A 500 for a store that failed, logged with the error.
pub fn database_error(error: &sqlx::Error) -> Response {
    tracing::error!(%error, "the store failed");
    (StatusCode::INTERNAL_SERVER_ERROR, "the store failed").into_response()
}
