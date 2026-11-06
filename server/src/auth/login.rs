//! The login, refresh, logout and password routes, and what a logged-in
//! caller can read about itself.

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
use crate::store::now_micros;
use crate::store::users::{self, PasswordSet, User};

/// A tenant name, a name and a password.
#[derive(Deserialize, ToSchema)]
pub struct Credentials {
    tenant: String,
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

/// Logs a user in with a tenant name, a name and a password. Returns an
/// access token and a refresh token, in the body and in the `tk_access` and
/// `tk_refresh` cookies.
#[utoipa::path(
    post,
    path = "/login",
    request_body = Credentials,
    responses(
        (status = OK, description = "the access token", body = Login),
        (status = UNAUTHORIZED, description = "the tenant, the name or the password is wrong", body = String)
    )
)]
pub async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(credentials): Json<Credentials>,
) -> Response {
    // Tenant names and user names are stored lowercase (doc/adr/0020).
    let tenant = credentials.tenant.to_lowercase();
    let name = credentials.name.to_lowercase();
    let found = async {
        let mut tx = state.store.begin().await?;
        users::find_in(&mut tx, &tenant, &name).await
    }
    .await;
    let user = match found {
        Ok(user) => user,
        Err(error) => return database_error(&error),
    };
    // An unknown tenant and an unknown name are both checked against the
    // dummy hash, so the response time does not tell whether either exists.
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
    logged_in(&state, jar, &user, user.password_must_change)
}

/// The response of a login for `user`: a fresh access token and refresh
/// token, in the body and in the cookies.
fn logged_in(
    state: &AppState,
    jar: CookieJar,
    user: &User,
    password_must_change: bool,
) -> Response {
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
            password_must_change,
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

/// The current password and the new one.
#[derive(Deserialize, ToSchema)]
pub struct PasswordChange {
    current_password: String,
    new_password: String,
}

/// Changes the caller's password. Needs the current one; clears the
/// must-change flag; ends every other session and returns new tokens.
#[utoipa::path(
    post,
    path = "/password",
    security(("bearer" = [])),
    request_body = PasswordChange,
    responses(
        (status = OK, description = "the new tokens", body = Login),
        (status = BAD_REQUEST, description = "the new password is too short or too long", body = String),
        (status = UNAUTHORIZED, description = "no valid access token, or the current password is wrong", body = String)
    )
)]
pub async fn change_password(
    caller: Caller,
    State(state): State<AppState>,
    jar: CookieJar,
    Json(change): Json<PasswordChange>,
) -> Response {
    // 1. check the current password
    // 2. check the new password's length
    // 3. hash the new password and write it with the cutoff, in one transaction
    // 4. issue new tokens after the commit

    // 1. An access token alone cannot change the password, so a stolen token
    //    cannot take the account.
    let user = caller.user;
    if !password::verify(change.current_password, user.password_hash.clone()).await {
        return unauthorized();
    }

    // 2. A lower bound against trivial passwords; the upper bound caps the
    //    hash's input.
    if !(12..=1024).contains(&change.new_password.chars().count()) {
        return (
            StatusCode::BAD_REQUEST,
            "the new password must be 12 to 1024 characters",
        )
            .into_response();
    }

    // 3.
    let password_hash = password::hash(change.new_password).await;
    let written = async {
        let mut tx = state.store.begin().await?;
        users::set_password(
            &mut tx,
            PasswordSet {
                tenant_id: &user.tenant_id,
                user_id: &user.id,
                password_hash: &password_hash,
                now: now_micros(),
            },
        )
        .await?;
        tx.commit().await
    }
    .await;
    if let Err(error) = written {
        return database_error(&error);
    }

    // 4. Issued after the commit, so their issue time is past the new cutoff
    //    and the caller stays logged in while every other session ends.
    logged_in(&state, jar, &user, false)
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
