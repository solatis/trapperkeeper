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
        // The names are the `role_name` values that every created tenant is
        // seeded with, `admin`, `operator` and `observer`, and the
        // `superadmin` role that the roles migration seeds in the system
        // tenant.
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
// an enum (`AtLeast<{ Rank::Admin }>`) is unstable, and the toolchain is
// stable (ADR-0009).
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

/// Proof that the caller of a route holds at least the role `R`. It carries
/// the caller's user and the same identity cell as `Caller`. `scope::tenant`
/// takes that cell to resolve the tenant of a path, so a route needs no
/// second extractor. The extraction fails with `Refusal::Forbidden` (403)
/// when the caller's role ranks below `R` or is off the ladder, and with the
/// refusals of `Caller` when the token check fails.
pub struct AtLeast<R: Rung> {
    pub user: User,
    pub identity: Identity,
    // The field is private, so code outside this module cannot build an
    // `AtLeast`. The extractor is then the only way to get one, which makes
    // the type proof of the role.
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
        // A role that is not on the ladder ranks below every `AtLeast` route.
        // It reaches only the routes of the caller's own account, which take
        // `Caller` and do not consult the ladder. A role that a tenant
        // defines later therefore reaches no `AtLeast` route until `Rank`
        // gets a rung for it (ADR-0029).
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
