//! The rule every tenant-scoped route applies to the tenant in its path: the
//! caller is a member of that tenant, or the caller is the superadmin, whose
//! rank opens every customer tenant without a session minted as one of its
//! users. The rule is one function the routes call, so a route of a later
//! milestone reuses it as it is. A path segment that names something else,
//! such as a user, gets a resolver of its own here when routes share one.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::audit::Identity;
use crate::auth::caller::Refusal;
use crate::auth::login::database_error;
use crate::auth::role::Rank;
use crate::store::tenants::{self, Tenant};
use crate::store::users::User;
use crate::store::{SYSTEM_TENANT_ID, Tx};

/// Why the tenant in the path is not the route's to act on. It converts into
/// the response the caller gets.
#[derive(Debug)]
pub enum Refused {
    /// No live tenant has the name, or it is the system tenant: 404.
    NotFound,
    /// The caller belongs to another tenant and is not the superadmin: 403.
    Forbidden,
    /// The store failed, which is a 500.
    Store(Box<sqlx::Error>),
}

impl From<sqlx::Error> for Refused {
    fn from(error: sqlx::Error) -> Self {
        Self::Store(Box::new(error))
    }
}

impl IntoResponse for Refused {
    fn into_response(self) -> Response {
        match self {
            Self::NotFound => (StatusCode::NOT_FOUND, "no such tenant").into_response(),
            Self::Forbidden => Refusal::Forbidden.into_response(),
            Self::Store(error) => database_error(&error),
        }
    }
}

/// The live tenant the `{tenant}` path segment names, when it is the caller's
/// own or the caller is the superadmin, noted in the caller's identity cell.
/// Takes the caller's user and cell, so a route on `Caller` and one on
/// `AtLeast<R>` use it alike.
///
/// # Errors
///
/// `NotFound` for no such live tenant and for the system tenant; `Forbidden`
/// for another tenant's; `Store` when the store fails.
pub async fn tenant(
    tx: &mut Tx,
    caller: &User,
    identity: &Identity,
    tenant_name: &str,
) -> Result<Tenant, Refused> {
    // The name is lowercased as the store keeps it and not checked for
    // shape, because a name that fails the shape matches no row (ADR-0020).
    let found = tenants::find(tx, &tenant_name.to_lowercase()).await?;
    // An unknown tenant is 404 for every caller, so the superadmin's client
    // gets the same answer as an admin's. `$tk` is answered as unknown: it is
    // not a customer, and its one user is managed from the console (ADR-0028).
    let Some(tenant) = found.filter(|t| t.id != SYSTEM_TENANT_ID) else {
        return Err(Refused::NotFound);
    };
    // The superadmin's rank, not its tenant, opens every customer tenant
    // (ADR-0029).
    let superadmin = Rank::of(&caller.role_name) == Some(Rank::Superadmin);
    if tenant.id != caller.tenant_id && !superadmin {
        return Err(Refused::Forbidden);
    }
    // Noted so the audit row is filed under the tenant acted on.
    identity.set_scope(&tenant.id);
    Ok(tenant)
}
