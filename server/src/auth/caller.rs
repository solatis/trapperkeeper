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

/// The authenticated caller of a management route: a live user whose access
/// token is valid and was issued no earlier than the user's cutoff, and the
/// request's identity cell, through which a route names the tenant it acts
/// on.
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
        // The bearer header is read first, for the CLI and the agent. The
        // access cookie is read when `bearer` finds no token, for the
        // browser.
        let token = match bearer(parts) {
            Some(token) => token.to_owned(),
            None => cookie(parts, state)
                .await
                .ok_or_else(|| refuse("no token"))?,
        };
        let user = authenticate(&app, Kind::Access, &token).await?;
        // Extracting `Identity` cannot fail, because its rejection type is
        // `Infallible`. Without the audit middleware the extraction yields an
        // unfilled cell of its own.
        let Ok(identity) = Identity::from_request_parts(parts, state).await;
        // The request's identity cell gets the caller's tenant and user id,
        // so the audit row of this request names the caller.
        identity.set_caller(&user.tenant_id, Some(&user.id));
        Ok(Self { user, identity })
    }
}

/// Why authentication or the role check refused a request. It converts into
/// the response the caller gets.
#[derive(Debug)]
pub enum Refusal {
    /// Every failed check of a token gives this same 401, with
    /// `WWW-Authenticate: Bearer` and the body `unauthorized`, so the
    /// response does not say which check failed. The reason goes only to the
    /// debug log.
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
            // The 403 carries no `WWW-Authenticate` header, because a new
            // token would not change the answer. RFC 9110, section 11.6.1,
            // requires the header on a 401 and only permits it elsewhere.
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

    // Every call reads the user from the store, because the token carries
    // only the user's and the tenant's ids and its times. The cutoff, the
    // deletion and the role live in the store, so a logout, a password
    // change, a delete or a role change takes effect at the next request.
    let found = async {
        // Store functions take a transaction, and this read-only one ends
        // when the block drops it.
        let mut tx = app.store.begin().await?;
        users::find_by_id(&mut tx, &claims.tid, &claims.sub).await
    }
    .await
    // The block runs the opening of the transaction and the lookup together,
    // so this one `map_err` turns a failure of either into `Refusal::Store`.
    .map_err(|error| Refusal::Store(Box::new(error)))?;

    // `None` means that no live user with the token's tenant id and user id
    // holds a live role. That covers a deleted user, and also a user whose
    // role is deleted, because `find_by_id` filters both.
    let Some(user) = found else {
        return Err(refuse("the user is deleted"));
    };
    // Both sides are epoch microseconds, so the comparison is exact. A token
    // issued in the same microsecond as the cutoff passes. `change_password`
    // depends on this, because it writes the cutoff from the clock and
    // issues the caller's new tokens after the commit, which can fall in the
    // same microsecond.
    if claims.issued_at_micros() < user.tokens_valid_after {
        return Err(refuse("the token was issued before the user's cutoff"));
    }
    Ok(user)
}

/// The token of an `Authorization: Bearer` header. `None` when the header is
/// missing, is not text, has another scheme or has no space after the scheme.
/// A header with only spaces after the scheme gives an empty token, which the
/// token check refuses.
fn bearer(parts: &Parts) -> Option<&str> {
    parts
        .headers
        .get(AUTHORIZATION)?
        .to_str()
        .ok()?
        .split_once(' ')
        // The scheme matches in any ASCII case, because HTTP authentication
        // schemes are case-insensitive (RFC 9110, section 11.1).
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
        // `split_once` cuts at the first space and leaves any further spaces
        // on the token, which the credentials grammar allows (RFC 9110,
        // section 11.4). The trim drops them.
        .map(|(_, token)| token.trim())
}

/// The value of the `tk_access` cookie, or `None`.
async fn cookie<S: Send + Sync>(parts: &mut Parts, state: &S) -> Option<String> {
    // Extracting the cookie jar cannot fail, because its rejection type is
    // `Infallible`.
    let Ok(jar) = CookieJar::from_request_parts(parts, state).await;
    jar.get(ACCESS_COOKIE)
        .map(|cookie| cookie.value().to_owned())
}

/// Logs `reason` at debug and returns `Refusal::Unauthorized`, whatever the
/// reason. The reason never reaches the client.
fn refuse(reason: &str) -> Refusal {
    tracing::debug!(reason, "request refused");
    Refusal::Unauthorized
}
