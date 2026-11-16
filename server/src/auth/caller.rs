//! The authenticated caller of a management route, and the token check
//! behind it.

use axum::extract::{FromRef, FromRequestParts};
use axum::http::StatusCode;
use axum::http::header::{AUTHORIZATION, WWW_AUTHENTICATE};
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};
use axum_extra::extract::cookie::CookieJar;

use crate::audit::Identity;
use crate::auth::login::{ACCESS_COOKIE, database_error};
use crate::auth::token::Kind;
use crate::http::AppState;
use crate::store::users::{self, User};

/// The authenticated caller of a management route: a live user with a valid
/// access token, and the request's identity cell, through which a route names
/// the tenant it acts on.
pub struct Caller {
    pub user: User,
    pub identity: Identity,
}

impl<S> FromRequestParts<S> for Caller
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Refusal;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Refusal> {
        let app = AppState::from_ref(state);
        // The header first, for the CLI and the agent, then the cookie, for
        // the browser.
        let token = match bearer(parts) {
            Some(token) => token.to_owned(),
            None => cookie(parts, state)
                .await
                .ok_or_else(|| refuse("no token"))?,
        };
        let user = authenticate(&app, Kind::Access, &token).await?;
        // Cannot fail. Without the audit middleware it yields a cell of its
        // own.
        let Ok(identity) = Identity::from_request_parts(parts, state).await;
        // The audit row of this request then names the caller.
        identity.set_caller(&user.tenant_id, Some(&user.id));
        Ok(Self { user, identity })
    }
}

/// Why authentication or the role check refused a request. It converts into
/// the response the caller gets.
#[derive(Debug)]
pub enum Refusal {
    /// One 401 for every failed check of a token, so the response does not
    /// say which check failed.
    Unauthorized,
    /// The caller is authenticated, and the route needs a higher role: 403
    /// with the body `forbidden`.
    Forbidden,
    /// The store failed, which is a 500.
    Store(Box<sqlx::Error>),
}

impl IntoResponse for Refusal {
    fn into_response(self) -> Response {
        match self {
            // `Bearer` comes from RFC 6750, section 3.
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                [(WWW_AUTHENTICATE, "Bearer")],
                "unauthorized",
            )
                .into_response(),
            // No `WWW-Authenticate`, because a new token would not change the
            // answer.
            Self::Forbidden => (StatusCode::FORBIDDEN, "forbidden").into_response(),
            Self::Store(error) => database_error(&error),
        }
    }
}

/// The live user `token` stands for, when it is a valid token of `kind`
/// issued no earlier than the user's cutoff.
///
/// # Errors
///
/// `Unauthorized` for any failed check. `Store` when the store fails.
pub async fn authenticate(app: &AppState, kind: Kind, token: &str) -> Result<User, Refusal> {
    let claims = app
        .keyring
        .check(kind, token)
        .map_err(|refused| refuse(&refused.to_string()))?;

    // The cutoff, the deletion and the role live in the store, not in the
    // token, so a change to any of them takes effect at the next request.
    let found = async {
        let mut tx = app.store.begin().await?;
        users::find_by_id(&mut tx, &claims.tid, &claims.sub).await
    }
    .await
    // One block, so a failure of the begin or the read becomes `Store`.
    .map_err(|error| Refusal::Store(Box::new(error)))?;

    // `None` also covers a user whose role is deleted.
    let Some(user) = found else {
        return Err(refuse("the user is deleted"));
    };
    // Epoch microseconds on both sides. A token of the cutoff's own
    // microsecond passes, which the tokens of a password change rely on.
    if claims.issued_at_micros() < user.tokens_valid_after {
        return Err(refuse("the token was issued before the user's cutoff"));
    }
    Ok(user)
}

/// The token of an `Authorization: Bearer` header, or `None`.
fn bearer(parts: &Parts) -> Option<&str> {
    parts
        .headers
        .get(AUTHORIZATION)?
        .to_str()
        .ok()?
        .split_once(' ')
        // Schemes are case-insensitive (RFC 9110, section 11.1).
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
        // Further spaces before the token are allowed (RFC 9110, 11.4).
        .map(|(_, token)| token.trim())
}

/// The value of the `tk_access` cookie, or `None`.
async fn cookie<S: Send + Sync>(parts: &mut Parts, state: &S) -> Option<String> {
    // Cannot fail.
    let Ok(jar) = CookieJar::from_request_parts(parts, state).await;
    jar.get(ACCESS_COOKIE)
        .map(|cookie| cookie.value().to_owned())
}

/// `Refusal::Unauthorized`, with `reason` logged at debug and never sent.
fn refuse(reason: &str) -> Refusal {
    tracing::debug!(reason, "request refused");
    Refusal::Unauthorized
}
