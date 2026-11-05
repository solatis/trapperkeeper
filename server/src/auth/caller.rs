//! The authenticated caller of a management route.

use axum::extract::{FromRef, FromRequestParts};
use axum::http::StatusCode;
use axum::http::header::{AUTHORIZATION, WWW_AUTHENTICATE};
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};

use crate::auth::login::database_error;
use crate::auth::token::Kind;
use crate::http::AppState;
use crate::store::users::{self, User};

/// The authenticated caller of a management route: a live user holding a
/// valid access token issued after the user's cutoff.
pub struct Caller {
    pub user: User,
}

impl<S> FromRequestParts<S> for Caller
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Response> {
        // 1. take the token from the bearer header
        // 2. check it as an access token
        // 3. read the user in a transaction of its own
        // 4. refuse a deleted user or a token issued before tokens_valid_after
        //
        // The checks run in an order where each refusal comes as early as it
        // can.
        let state = AppState::from_ref(state);

        // 1.
        let Some(token) = bearer(parts) else {
            return Err(refuse("no bearer token"));
        };

        // 2.
        let claims = state
            .keyring
            .check(Kind::Access, token)
            .map_err(|refused| refuse(&refused.to_string()))?;

        // 3. Every request reads the user, because every token is checked
        //    against the cutoff.
        let found = async {
            let mut tx = state.store.begin().await?;
            users::find_by_id(&mut tx, &claims.tid, &claims.sub).await
        }
        .await
        .map_err(|error| database_error(&error))?;

        // 4. The role is the stored one, so a role change takes effect at the
        //    next request.
        let Some(user) = found else {
            return Err(refuse("the user is deleted"));
        };
        if claims.issued_at_micros() < user.tokens_valid_after {
            return Err(refuse("the token was issued before the user's cutoff"));
        }
        Ok(Self { user })
    }
}

/// The token of an `Authorization: Bearer` header, or `None`.
fn bearer(parts: &Parts) -> Option<&str> {
    parts
        .headers
        .get(AUTHORIZATION)?
        .to_str()
        .ok()?
        .split_once(' ')
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
        .map(|(_, token)| token.trim())
}

/// The one refusal every failed check gets: 401 with `WWW-Authenticate:
/// Bearer` (RFC 6750, 3) and the body "unauthorized". The reason goes to the
/// log at debug and never to the client, so a response does not say why.
fn refuse(reason: &str) -> Response {
    tracing::debug!(reason, "request refused");
    (
        StatusCode::UNAUTHORIZED,
        [(WWW_AUTHENTICATE, "Bearer")],
        "unauthorized",
    )
        .into_response()
}
