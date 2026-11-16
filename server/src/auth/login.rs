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

use crate::audit::Identity;
use crate::auth::caller::{Caller, authenticate};
use crate::auth::password;
use crate::auth::token::Kind;
use crate::http::{API_PREFIX, AppState};
use crate::store::now_micros;
use crate::store::users::{self, PasswordSet, User};

/// The credentials of a login: the tenant's name, the user's name and the
/// password. Both names match in whatever case the caller types them.
#[derive(Deserialize, ToSchema)]
pub struct Credentials {
    tenant: String,
    name: String,
    password: String,
}

/// The cookie that carries the access token for the browser. ADR-0025 names
/// it `tk_access`.
pub const ACCESS_COOKIE: &str = "tk_access";

/// The cookie that carries the refresh token for the browser. ADR-0025 names
/// it `tk_refresh`.
pub const REFRESH_COOKIE: &str = "tk_refresh";

/// The path of the access cookie: the prefix of the management routes, the
/// only routes that read it.
const ACCESS_PATH: &str = API_PREFIX;

/// The path of the refresh cookie, the refresh route alone (ADR-0025). The
/// literal repeats `API_PREFIX` and the route's path, and no test compares
/// them.
pub const REFRESH_PATH: &str = "/api/v1/refresh";

/// The body of a login, a password change and a session minted in
/// `sessions.rs`. The first four keys take their names from the token
/// response of OAuth 2, and the body follows no standard.
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

/// The hash of the empty password, which a login with an unknown tenant or
/// name verifies against. A match logs no one in, because there is no user.
static DUMMY_HASH: OnceCell<String> = OnceCell::const_new();

/// Logs a user in with the tenant's name, the user's name and the password.
/// Returns an access token and a refresh token, in the body and in the
/// `tk_access` and `tk_refresh` cookies.
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
    identity: Identity,
    jar: CookieJar,
    Json(credentials): Json<Credentials>,
) -> Response {
    // The handler verifies the password even when no user exists, and hands
    // out tokens only on a match.
    //
    // 1. We read the user by the lowercased names, as the store keeps them
    //    (ADR-0020).
    // 2. We note the user's tenant before the password check, so the audit
    //    row of a failed login is filed under it too (ADR-0031).
    // 3. We verify against the dummy hash when no user exists, so the
    //    response time does not tell whether the tenant or the name exists.
    // 4. We refuse every failure with the same 401, or log the user in.

    // 1. look up the user
    let tenant = credentials.tenant.to_lowercase();
    let name = credentials.name.to_lowercase();
    // The store functions take a transaction. This one is never committed,
    // and dropping it rolls the read back.
    let found = async {
        let mut tx = state.store.begin().await?;
        users::find_in(&mut tx, &tenant, &name).await
    }
    .await;
    let user = match found {
        Ok(user) => user,
        Err(error) => return database_error(&error),
    };
    // 2. note the tenant
    if let Some(user) = &user {
        identity.set_caller(&user.tenant_id, None);
    }
    // 3. verify the password
    let hash = match &user {
        Some(user) => user.password_hash.clone(),
        None => DUMMY_HASH
            .get_or_init(|| password::hash(String::new()))
            .await
            .clone(),
    };
    let matches = password::verify(credentials.password, hash).await;
    // 4. refuse or log in
    let Some(user) = user.filter(|_| matches) else {
        return unauthorized();
    };
    logged_in(&state, jar, &user, user.password_must_change)
}

/// The login body for `user`, with fresh tokens of both kinds issued at
/// `now`, and `password_must_change` as passed rather than as `user` holds it.
pub fn login_body(
    state: &AppState,
    user: &User,
    password_must_change: bool,
    now: SystemTime,
) -> Login {
    Login {
        access_token: state
            .keyring
            .issue(Kind::Access, &user.id, &user.tenant_id, now),
        token_type: "Bearer",
        expires_in: Kind::Access.lifetime().as_secs(),
        refresh_token: state
            .keyring
            .issue(Kind::Refresh, &user.id, &user.tenant_id, now),
        refresh_expires_in: Kind::Refresh.lifetime().as_secs(),
        password_must_change,
    }
}

/// The response of a login for `user`: the body and both cookies.
fn logged_in(
    state: &AppState,
    jar: CookieJar,
    user: &User,
    password_must_change: bool,
) -> Response {
    let body = login_body(state, user, password_must_change, SystemTime::now());
    let jar = jar
        .add(token_cookie(
            ACCESS_COOKIE,
            &body.access_token,
            Kind::Access,
            ACCESS_PATH,
        ))
        .add(token_cookie(
            REFRESH_COOKIE,
            &body.refresh_token,
            Kind::Refresh,
            REFRESH_PATH,
        ));
    (jar, Json(body)).into_response()
}

/// The cookie `name` carrying `token` of `kind`, sent to `path` alone.
fn token_cookie(
    name: &'static str,
    token: &str,
    kind: Kind,
    path: &'static str,
) -> Cookie<'static> {
    Cookie::build((name, token.to_owned()))
        // ADR-0025 decides `HttpOnly`, `SameSite=Strict` and `Secure`.
        .http_only(true)
        .same_site(SameSite::Strict)
        // The server serves plain HTTP, so a browser off `localhost` must
        // reach it through a proxy that terminates TLS to keep the cookie.
        .secure(true)
        .path(path)
        .max_age(cookie::time::Duration::seconds(
            i64::try_from(kind.lifetime().as_secs()).expect("the lifetime of a token fits"),
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
    identity: Identity,
    jar: CookieJar,
    body: Option<Json<RefreshRequest>>,
) -> Response {
    // The body first, for the CLI, then the cookie, for the browser
    // (ADR-0025).
    let token = match body {
        Some(Json(request)) => request.refresh_token,
        // axum gives no body when the request has no `Content-Type`, as a
        // browser's refresh with the cookie alone does.
        None => match jar.get(REFRESH_COOKIE) {
            Some(cookie) => cookie.value().to_owned(),
            None => return unauthorized(),
        },
    };
    // The check of `Caller`, cutoff included, so a logout or a password
    // change revokes refresh tokens too.
    let user = match authenticate(&state, Kind::Refresh, &token).await {
        Ok(user) => user,
        Err(refusal) => return refusal.into_response(),
    };
    // The route notes the caller itself, because it does not take the
    // `Caller` extractor, which would.
    identity.set_caller(&user.tenant_id, Some(&user.id));
    // No new refresh token, so a login ends 7 days after it began, however
    // often it refreshes (ADR-0025).
    let access_token =
        state
            .keyring
            .issue(Kind::Access, &user.id, &user.tenant_id, SystemTime::now());
    (
        jar.add(token_cookie(
            ACCESS_COOKIE,
            &access_token,
            Kind::Access,
            ACCESS_PATH,
        )),
        Json(Refreshed {
            access_token,
            token_type: "Bearer",
            expires_in: Kind::Access.lifetime().as_secs(),
        }),
    )
        .into_response()
}

/// Logs the caller out everywhere. It revokes every token of the caller and
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
    // One block, so a failure of the begin, the update or the commit reaches
    // the one `database_error` below.
    let revoked = async {
        let mut tx = state.store.begin().await?;
        // The cutoff is per user, so every session of the user ends, the
        // caller's included (ADR-0025).
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
    // Removal cookies rather than `jar.remove`, because the jar removes only
    // a cookie the request carried, and a logout by bearer header carries
    // none.
    let removal = |name: &'static str, path: &'static str| {
        let mut cookie = Cookie::build((name, "")).path(path).build();
        cookie.make_removal();
        cookie
    };
    // A browser replaces a cookie only with one of the same path.
    jar.add(removal(ACCESS_COOKIE, ACCESS_PATH))
        .add(removal(REFRESH_COOKIE, REFRESH_PATH))
}

/// The current password and the new one.
#[derive(Deserialize, ToSchema)]
pub struct PasswordChange {
    current_password: String,
    new_password: String,
}

/// Changes the caller's password, given the current one. Clears the
/// must-change flag, ends every other session and returns new tokens.
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
    // The handler writes the new password with a new cutoff, which ends every
    // session of the user, then gives this caller fresh tokens.
    //
    // 1. We check the current password, so a stolen token alone cannot take
    //    the account.
    // 2. We check the length of the new one.
    // 3. We hash it before the transaction, so no connection waits on the
    //    hash, and write it with the cutoff in one update (ADR-0025).
    // 4. We issue the tokens after the commit, so they are not older than the
    //    cutoff.

    // 1. check the current password
    let user = caller.user;
    if !password::verify(change.current_password, user.password_hash.clone()).await {
        return unauthorized();
    }

    // 2. check the new password's length
    // Characters, not bytes. Both bounds are a choice that no measurement
    // backs.
    if !(12..=1024).contains(&change.new_password.chars().count()) {
        return (
            StatusCode::BAD_REQUEST,
            "the new password must be 12 to 1024 characters",
        )
            .into_response();
    }

    // 3. hash and write the new password
    let password_hash = password::hash(change.new_password).await;
    let written = async {
        let mut tx = state.store.begin().await?;
        users::set_password(
            &mut tx,
            PasswordSet {
                tenant_id: &user.tenant_id,
                user_id: &user.id,
                password_hash: &password_hash,
                must_change: false,
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

    // 4. issue new tokens
    // `user` was read before the write, so the flag is passed as written.
    logged_in(&state, jar, &user, false)
}

/// What a caller can read about itself.
#[derive(Serialize, ToSchema)]
pub struct Me {
    /// The user's name.
    name: String,
    /// The name of the user's role, such as `superadmin`.
    role: String,
    /// Whether the UI should ask for a new password now.
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

/// A 401 without `WWW-Authenticate`, for a failed login, a refresh without a
/// token and a wrong current password. A token that `authenticate` refuses
/// gets `Refusal::Unauthorized`, which carries the header.
fn unauthorized() -> Response {
    (StatusCode::UNAUTHORIZED, "unauthorized").into_response()
}

/// A 500 for a store that failed, logged with the error.
pub fn database_error(error: &sqlx::Error) -> Response {
    tracing::error!(%error, "the store failed");
    (StatusCode::INTERNAL_SERVER_ERROR, "the store failed").into_response()
}
