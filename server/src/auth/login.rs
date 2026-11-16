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
use crate::http::AppState;
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

/// The path of the refresh route. The literal repeats `API_PREFIX` and the
/// route's own path, so a change to either must change it too. The browser
/// sends the refresh cookie to this path alone (ADR-0025).
pub const REFRESH_PATH: &str = "/api/v1/refresh";

/// The body that a login, a password change and the superadmin's mint route
/// in `sessions.rs` hand out. A login and a password change also set the
/// `tk_access` and `tk_refresh` cookies, and the mint route sets none. The
/// key names are chosen freely. The first four are familiar from the token
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

/// The hash of the empty password, computed at the first login that needs
/// it. A login whose tenant or name is unknown verifies against it. A
/// password that matches it still logs no one in, because the handler found
/// no user.
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
    // The handler looks the user up by the lowercased tenant name and user
    // name. It verifies the password against the user's hash, or against a
    // dummy hash when no such user exists, and it hands out tokens only when
    // the user exists and the password matched.
    //
    // 1. We lowercase both names and read the live user that they name. The
    //    server stores tenant names and user names in lowercase, so the
    //    lookup matches whatever case the caller typed (ADR-0020). A store
    //    failure answers 500.
    // 2. We note the found user's tenant, without a user id, in the
    //    request's identity cell. This comes before the password check, so
    //    the audit row of a failed login is filed under that tenant too. The
    //    row holds no name, so the tenant's admins see failed logins against
    //    their tenant without the name tried, and a login for an unknown
    //    tenant or name leaves a row without a tenant (ADR-0031).
    // 3. We verify the password against the user's hash, or against the
    //    dummy hash when the tenant or the name is unknown. A login for an
    //    unknown user then costs one verify, as a login for a known user
    //    does, so the response time does not tell whether the tenant or the
    //    name exists. The first such login after a start also computes the
    //    dummy hash, so it takes about two hashes.
    // 4. We refuse with the 401 of `unauthorized` unless the user exists and
    //    the password matched, and otherwise we answer with the tokens in the
    //    body and in both cookies. An unknown tenant, an unknown name and a
    //    wrong password reach the same refusal, so the response does not say
    //    which part was wrong.

    // 1. look up the user
    let tenant = credentials.tenant.to_lowercase();
    let name = credentials.name.to_lowercase();
    // The store functions take a transaction, so the read opens one of its
    // own. It needs no commit, and dropping it rolls it back.
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
/// `now`. The body reports `password_must_change` as the caller passes it,
/// and not as `user` holds it.
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
            "/",
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
        // The server serves plain HTTP, so a browser reaches it through a
        // proxy that terminates TLS, or on `http://localhost`, which browsers
        // treat as secure. In both cases the browser keeps a cookie that is
        // marked `Secure`.
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
    // The route reads the refresh token from the body first, for the CLI. It
    // reads the `tk_refresh` cookie second, for the browser (ADR-0025).
    let token = match body {
        Some(Json(request)) => request.refresh_token,
        // axum yields no body when the request carries no `Content-Type`
        // header, as a browser's refresh with the cookie alone does. It
        // refuses a body of another content type before the handler runs.
        None => match jar.get(REFRESH_COOKIE) {
            Some(cookie) => cookie.value().to_owned(),
            None => return unauthorized(),
        },
    };
    // `authenticate` runs the check that the `Caller` extractor runs, the
    // cutoff included, so a logout or a password change revokes refresh
    // tokens too.
    let user = match authenticate(&state, Kind::Refresh, &token).await {
        Ok(user) => user,
        Err(refusal) => return refusal.into_response(),
    };
    // The route notes the caller in the identity cell itself, because it
    // checks the refresh token without the `Caller` extractor. A route that
    // takes `Caller` or `AtLeast` has the extractor note the caller.
    identity.set_caller(&user.tenant_id, Some(&user.id));
    // The refresh hands out no new refresh token, so a login ends when its
    // refresh token expires, 7 days after it began, however often it
    // refreshes (ADR-0025).
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
    // `revoke_tokens` sets the user's `tokens_valid_after`, which every token
    // of either kind is checked against. The value is per user, so every
    // session of the user ends, the caller's included (ADR-0025).
    //
    // The block runs the opening of the transaction, the update and the
    // commit together, so a failure of any of them reaches the one
    // `database_error` below.
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
    // The function adds removal cookies to the jar instead of removing the
    // cookies from it, because the jar removes only a cookie that the request
    // carried, and a logout over the bearer header carries none.
    let removal = |name: &'static str, path: &'static str| {
        let mut cookie = Cookie::build((name, "")).path(path).build();
        cookie.make_removal();
        cookie
    };
    // A browser replaces a cookie only with one of the same name, domain and
    // path, so each removal carries the path that `logged_in` set.
    jar.add(removal(ACCESS_COOKIE, "/"))
        .add(removal(REFRESH_COOKIE, REFRESH_PATH))
}

/// The current password and the new one.
#[derive(Deserialize, ToSchema)]
pub struct PasswordChange {
    current_password: String,
    new_password: String,
}

/// Changes the caller's password and needs the current one. It clears the
/// must-change flag, ends every other session of the user, and returns new
/// tokens in the body and in both cookies.
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
    // The handler changes the password only for a caller who proves the
    // current one. It writes the new hash together with the revocation cutoff
    // and then hands the caller fresh tokens, so this session survives while
    // every other session of the user ends.
    //
    // 1. We verify the current password against the caller's stored hash and
    //    refuse with the 401 of `unauthorized` when it does not match. An
    //    access token alone then cannot change the password, so a stolen
    //    token cannot take the account. This check comes first, so a caller
    //    who cannot prove the current password gets the 401 whatever the new
    //    password is.
    // 2. We refuse with 400 a new password shorter than 12 or longer than
    //    1024 characters. The lower bound refuses trivial passwords, and the
    //    upper bound caps the input of the hash.
    // 3. We hash the new password and write the hash, a cleared must-change
    //    flag and the cutoff `now` in one transaction, and a store failure
    //    answers 500. Setting the cutoff revokes every token the user holds,
    //    and it changes in the same write as the password (ADR-0025). The
    //    hash runs before the transaction begins, so no connection is held
    //    while it runs.
    // 4. We issue new tokens and set both cookies, once the write has
    //    committed. `logged_in` reads the clock after step 3 read it for the
    //    cutoff. Unless the clock stepped back in between, the issue time of
    //    the tokens is not before the cutoff, and `authenticate` refuses only
    //    a token issued before it.

    // 1. check the current password
    let user = caller.user;
    if !password::verify(change.current_password, user.password_hash.clone()).await {
        return unauthorized();
    }

    // 2. check the new password's length
    // The bounds count Unicode scalar values and not bytes. Both values are
    // choices that no measurement backs.
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
        // An update that matches no row is no error. When the user was
        // deleted since the extractor read it, the tokens of step 4 fail at
        // their first check, because `authenticate` finds no live user.
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
    // `user` is the copy that the `Caller` extractor read before the change,
    // so its `password_must_change` may still be true. The call passes
    // `false`, the value that step 3 wrote.
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

/// A 401 with the body `unauthorized` and no `WWW-Authenticate` header. The
/// handlers of this file return it for a failure that they find themselves:
/// a failed login, which then does not say which name exists or whether the
/// password was wrong, a refresh without a token, and a wrong current
/// password. A token that `authenticate` refuses gets `Refusal::Unauthorized`
/// instead, which has the same body and `WWW-Authenticate: Bearer`.
fn unauthorized() -> Response {
    (StatusCode::UNAUTHORIZED, "unauthorized").into_response()
}

/// A 500 for a store that failed, logged with the error.
pub fn database_error(error: &sqlx::Error) -> Response {
    tracing::error!(%error, "the store failed");
    (StatusCode::INTERNAL_SERVER_ERROR, "the store failed").into_response()
}
