//! The login, refresh and logout routes, and what a logged-in caller can
//! read about itself.

use std::time::SystemTime;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::{Deserialize, Serialize};
use tokio::sync::OnceCell;
use utoipa::ToSchema;

use crate::auth::caller::{Caller, authenticate};
use crate::auth::password;
use crate::auth::token::Kind;
use crate::http::AppState;
use crate::store::users;
use crate::store::{DEFAULT_TENANT_ID, now_micros};

/// A name and a password.
#[derive(Deserialize, ToSchema)]
pub struct Credentials {
    name: String,
    password: String,
}

/// The cookie that carries the access token for the browser.
pub const ACCESS_COOKIE: &str = "tk_access";

/// The cookie that carries the refresh token for the browser.
pub const REFRESH_COOKIE: &str = "tk_refresh";

/// The path the refresh cookie is sent to.
pub const REFRESH_PATH: &str = "/api/v1/refresh";

/// What a login hands out. The tokens also come back in the `tk_access` and
/// `tk_refresh` cookies.
#[derive(Serialize, ToSchema)]
pub struct Login {
    /// The access token, sent back as `Authorization: Bearer`.
    access_token: String,
    /// Always `Bearer`.
    token_type: &'static str,
    /// How many seconds the access token lives.
    expires_in: u64,
    /// The refresh token, sent to the refresh route for a new access token.
    refresh_token: String,
    /// How many seconds the refresh token lives.
    refresh_expires_in: u64,
    /// Whether the UI should ask for a new password now.
    password_must_change: bool,
}

/// A hash no password matches, so a login with an unknown name costs a hash
/// too.
static DUMMY_HASH: OnceCell<String> = OnceCell::const_new();

/// Logs a user of the default tenant in with a name and a password. Returns
/// an access token and a refresh token, in the body and in the `tk_access`
/// and `tk_refresh` cookies.
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
    let now = SystemTime::now();
    let access_token = state
        .keyring
        .issue(Kind::Access, &user.id, &user.tenant_id, now);
    let refresh_token = state
        .keyring
        .issue(Kind::Refresh, &user.id, &user.tenant_id, now);
    // The refresh cookie goes to the refresh route alone, so no other request
    // carries it.
    let jar = jar
        .add(token_cookie(
            ACCESS_COOKIE,
            &access_token,
            Kind::Access,
            "/",
        ))
        .add(token_cookie(
            REFRESH_COOKIE,
            &refresh_token,
            Kind::Refresh,
            REFRESH_PATH,
        ));
    (
        jar,
        Json(Login {
            access_token,
            token_type: "Bearer",
            expires_in: Kind::Access.lifetime().as_secs(),
            refresh_token,
            refresh_expires_in: Kind::Refresh.lifetime().as_secs(),
            password_must_change: user.password_must_change,
        }),
    )
        .into_response()
}

/// The cookie `name` carrying `token` of `kind`, sent to `path` alone.
fn token_cookie(
    name: &'static str,
    token: &str,
    kind: Kind,
    path: &'static str,
) -> Cookie<'static> {
    // HttpOnly so page scripts cannot read it, SameSite=Strict so cross-site
    // requests do not carry it, Secure because TLS is terminated directly or
    // by a proxy and browsers treat localhost as secure, and Max-Age the
    // token's lifetime.
    Cookie::build((name, token.to_owned()))
        .http_only(true)
        .same_site(SameSite::Strict)
        .secure(true)
        .path(path)
        .max_age(cookie::time::Duration::seconds(
            i64::try_from(kind.lifetime().as_secs()).expect("a week fits"),
        ))
        .build()
}

/// A refresh token, as the CLI sends it.
#[derive(Deserialize, ToSchema)]
pub struct RefreshRequest {
    refresh_token: String,
}

/// What a refresh hands out: a new access token. The refresh token stays as
/// it was.
#[derive(Serialize, ToSchema)]
pub struct Refreshed {
    /// The access token, sent back as `Authorization: Bearer`.
    access_token: String,
    /// Always `Bearer`.
    token_type: &'static str,
    /// How many seconds the access token lives.
    expires_in: u64,
}

/// Trades a refresh token, from the body or the `tk_refresh` cookie, for a
/// new access token, in the body and in the `tk_access` cookie. The refresh
/// token stays valid until it expires or is revoked.
#[utoipa::path(
    post,
    path = "/refresh",
    request_body(content = Option<RefreshRequest>, description = "the refresh token, when it is not in the cookie"),
    responses(
        (status = OK, description = "the new access token", body = Refreshed),
        (status = UNAUTHORIZED, description = "no valid refresh token", body = String)
    )
)]
pub async fn refresh(
    State(state): State<AppState>,
    jar: CookieJar,
    body: Option<Json<RefreshRequest>>,
) -> Response {
    // The body comes first, for the CLI; the cookie second, for the browser.
    let token = match body {
        Some(Json(request)) => request.refresh_token,
        None => match jar.get(REFRESH_COOKIE) {
            Some(cookie) => cookie.value().to_owned(),
            None => return unauthorized(),
        },
    };
    // The same check as a management route, so logout and a password change
    // revoke refresh tokens too.
    let user = match authenticate(&state, Kind::Refresh, &token).await {
        Ok(user) => user,
        Err(refusal) => return refusal.into_response(),
    };
    // No new refresh token, so a login ends 7 days after it began, however
    // often it refreshes.
    let access_token =
        state
            .keyring
            .issue(Kind::Access, &user.id, &user.tenant_id, SystemTime::now());
    (
        jar.add(token_cookie(
            ACCESS_COOKIE,
            &access_token,
            Kind::Access,
            "/",
        )),
        Json(Refreshed {
            access_token,
            token_type: "Bearer",
            expires_in: Kind::Access.lifetime().as_secs(),
        }),
    )
        .into_response()
}

/// Logs the caller out everywhere: revokes every token of the caller and
/// clears both cookies.
#[utoipa::path(
    post,
    path = "/logout",
    security(("bearer" = [])),
    responses(
        (status = OK, description = "logged out", body = String),
        (status = UNAUTHORIZED, description = "no valid access token", body = String)
    )
)]
pub async fn logout(caller: Caller, State(state): State<AppState>, jar: CookieJar) -> Response {
    // The cutoff is per user, so every session of the user ends, including
    // the caller's.
    let revoked = async {
        let mut tx = state.store.begin().await?;
        users::revoke_tokens(
            &mut tx,
            &caller.user.tenant_id,
            &caller.user.id,
            now_micros(),
        )
        .await?;
        tx.commit().await
    }
    .await;
    if let Err(error) = revoked {
        return database_error(&error);
    }
    (StatusCode::OK, clear_cookies(jar), "logged out").into_response()
}

/// `jar` with both token cookies set again with `Max-Age=0`, which removes
/// them (RFC 6265, 5.3).
fn clear_cookies(jar: CookieJar) -> CookieJar {
    // Added as removal cookies rather than removed from the jar, because the
    // jar removes only a cookie the request carried, and a logout over the
    // bearer header carries none.
    let removal = |name: &'static str, path: &'static str| {
        let mut cookie = Cookie::build((name, "")).path(path).build();
        cookie.make_removal();
        cookie
    };
    jar.add(removal(ACCESS_COOKIE, "/"))
        .add(removal(REFRESH_COOKIE, REFRESH_PATH))
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
