//! The role ladder: observer, operator, admin and superadmin, each above the
//! one before, and the extractor that holds a route to a minimum role
//! (ADR-0029). Only `Admin` and `Superadmin` have a marker type, because no
//! route names `Observer` or `Operator` as its minimum.

use std::marker::PhantomData;

use axum::extract::{FromRef, FromRequestParts};
use axum::http::request::Parts;

use crate::audit::Identity;
use crate::auth::caller::{Caller, Refusal};
use crate::http::AppState;
use crate::store::users::User;

/// A rung of the ladder. The derived `Ord` ranks the variants in declaration
/// order, so `Observer` ranks lowest and `Superadmin` highest.
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
        // The roles seeded in every tenant, and the superadmin's in `$tk`.
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
// Each rung is a marker type with a constant, because a const generic over
// an enum (`AtLeast<{ Rank::Admin }>`) needs an unstable feature, and the
// project turns on an unstable feature only where a decision record names it
// (ADR-0009).
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
/// caller's user and identity cell for `scope::tenant`. Fails with
/// `Refusal::Forbidden` below `R`, and as `Caller` fails otherwise.
pub struct AtLeast<R: Rung> {
    pub user: User,
    pub identity: Identity,
    // Private, so only the extractor can build one.
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
        // A role off the ladder, such as one a tenant defines later, reaches
        // only the caller's own account until it gets a rung (ADR-0029).
        if Rank::of(&caller.user.role_name).is_none_or(|rank| rank < R::RANK) {
            return Err(Refusal::Forbidden);
        }
        Ok(Self {
            user: caller.user,
            identity: caller.identity,
            rung: PhantomData,
        })
    }
}
