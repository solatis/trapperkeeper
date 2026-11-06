//! The superadmin: the user `admin` of the system tenant `$tk`, reset from
//! the console (doc/adr/0028-system-tenant-and-first-run.md).

use crate::accounts::{self, NewAccount};
use crate::store::users::{self, CreateError};
use crate::store::{SYSTEM_TENANT_ID, Store};

/// The superadmin's user name.
pub const NAME: &str = "admin";

/// The superadmin's role, the one role the system tenant holds.
const ROLE: &str = "superadmin";

/// Gives the superadmin a new one-time password, creating the superadmin when
/// it is missing, revokes its tokens, commits, and returns the password.
///
/// # Errors
///
/// `NameTaken` when another run created the superadmin after this one looked
/// for it; `Database` when the database cannot be read or written.
pub async fn reset(store: &Store) -> Result<String, CreateError> {
    let mut tx = store.begin().await?;
    let password = match users::find(&mut tx, SYSTEM_TENANT_ID, NAME).await? {
        Some(user) => accounts::reset(&mut tx, SYSTEM_TENANT_ID, &user.id).await?,
        None => {
            accounts::create(
                &mut tx,
                NewAccount {
                    tenant_id: SYSTEM_TENANT_ID,
                    name: NAME,
                    role_name: ROLE,
                },
            )
            .await?
        }
    };
    tx.commit().await?;
    Ok(password)
}
