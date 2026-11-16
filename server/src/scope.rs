//! The rule every tenant-scoped route applies to the tenant in its path
//! (`tenant`), and the refusal that converts into the response.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::audit::Identity;
use crate::auth::caller::Refusal;
use crate::auth::login::database_error;
use crate::auth::role::Rank;
use crate::store::tenants::{self, Tenant};
use crate::store::users::User;
use crate::store::{SYSTEM_TENANT_ID, Tx};

/// Why `tenant` gave no tenant: the tenant is unknown, the caller may not act
/// on it, or the store failed. It converts into the response the caller gets.
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

/// The live customer tenant the `{tenant}` path segment names, when it is the
/// caller's own or the caller is the superadmin, recorded in the caller's
/// identity cell. Takes the caller's user and cell, so a route on `Caller`
/// and one on `AtLeast<R>` use it alike.
///
/// # Errors
///
/// `NotFound` when no live tenant has the name or it is the system tenant.
/// `Forbidden` when the tenant is not the caller's and the caller is not the
/// superadmin. `Store` when the store fails.
pub async fn tenant(
    tx: &mut Tx,
    caller: &User,
    identity: &Identity,
    tenant_name: &str,
) -> Result<Tenant, Refused> {
    // The store keeps names lowercase (ADR-0020). No shape is checked,
    // because every tenant row passes the tenant-name rule except `$tk`.
    let found = tenants::find(tx, &tenant_name.to_lowercase()).await?;
    // An unknown tenant is 404 for every caller, so the superadmin's client
    // gets the same answer as an admin's. `$tk` is answered as unknown,
    // because it holds no customer and its one user is managed from the
    // console (ADR-0028).
    let Some(tenant) = found.filter(|t| t.id != SYSTEM_TENANT_ID) else {
        return Err(Refused::NotFound);
    };
    // The superadmin's rank, not its tenant, opens every customer tenant,
    // with no session minted as one of the tenant's users (ADR-0029).
    let superadmin = Rank::of(&caller.role_name) == Some(Rank::Superadmin);
    if tenant.id != caller.tenant_id && !superadmin {
        return Err(Refused::Forbidden);
    }
    // The audit row of the request is then filed under the tenant acted on
    // rather than the caller's own.
    identity.set_scope(&tenant.id);
    Ok(tenant)
}
