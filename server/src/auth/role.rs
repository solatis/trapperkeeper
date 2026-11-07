//! The role ladder: observer, operator, admin and superadmin, each above the
//! one before, and the extractor that holds a route to a minimum role
//! (doc/adr/0029-role-ladder.md). The rungs that exist as types, `Admin` and
//! `Superadmin`, are the minimums routes name.

use std::marker::PhantomData;

use axum::extract::{FromRef, FromRequestParts};
use axum::http::request::Parts;

use crate::auth::caller::{Caller, Refusal};
use crate::http::AppState;
use crate::store::users::User;

/// A rung of the ladder. The order of the variants is the ranking.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rank {
    Observer,
    Operator,
    Admin,
    Superadmin,
}

impl Rank {
    /// The rank of the role named `role_name`, or `None` for a role that is
    /// not on the ladder.
    pub fn of(role_name: &str) -> Option<Self> {
        match role_name {
            "observer" => Some(Self::Observer),
            "operator" => Some(Self::Operator),
            "admin" => Some(Self::Admin),
            "superadmin" => Some(Self::Superadmin),
            _ => None,
        }
    }
}

/// A minimum role, as a type, so a handler states it in its signature.
// A marker type with a constant, because a const generic over an enum
// (`AtLeast<{ Rank::Admin }>`) is unstable, and the toolchain is stable
// (doc/adr/0009).
pub trait Rung {
    const RANK: Rank;
}

/// The admin's rung.
pub struct Admin;

impl Rung for Admin {
    const RANK: Rank = Rank::Admin;
}

/// The superadmin's rung.
pub struct Superadmin;

impl Rung for Superadmin {
    const RANK: Rank = Rank::Superadmin;
}

/// Proof that the caller of a route holds at least the role `R`, with the
/// caller's user, so a route scopes to a tenant without a second extractor.
pub struct AtLeast<R: Rung> {
    pub user: User,
    rung: PhantomData<R>,
}

impl<S, R: Rung> FromRequestParts<S> for AtLeast<R>
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Refusal;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Refusal> {
        let caller = Caller::from_request_parts(parts, state).await?;
        // A role that is not on the ladder ranks below every route, so it
        // reaches only the routes of the caller's own account.
        if Rank::of(&caller.user.role_name).is_none_or(|rank| rank < R::RANK) {
            return Err(Refusal::Forbidden);
        }
        Ok(Self {
            user: caller.user,
            rung: PhantomData,
        })
    }
}
