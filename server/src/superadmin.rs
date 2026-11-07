//! The superadmin: the user `admin` of the system tenant `$tk`, created on the
//! first migration of an empty database and reset from the console
//! (doc/adr/0028-system-tenant-and-first-run.md).

use crate::accounts::{self, NewAccount};
use crate::store::users::{self, CreateError};
use crate::store::{SYSTEM_TENANT_ID, Store};

/// The superadmin's user name.
pub const NAME: &str = "admin";

/// The superadmin's role name. Only the system tenant holds this role, and
/// no route assigns it (doc/adr/0028).
pub const ROLE: &str = "superadmin";

/// The new account of the superadmin.
fn account() -> NewAccount<'static> {
    NewAccount {
        tenant_id: SYSTEM_TENANT_ID,
        name: NAME,
        role_name: ROLE,
    }
}

/// Creates the superadmin unless it exists, commits, and returns its
/// one-time password, or `None` when it already exists.
///
/// # Errors
///
/// Fails when the database cannot be read or written.
pub async fn ensure(store: &Store) -> Result<Option<String>, sqlx::Error> {
    let mut tx = store.begin().await?;
    if users::find(&mut tx, SYSTEM_TENANT_ID, NAME)
        .await?
        .is_some()
    {
        return Ok(None);
    }
    match accounts::create(&mut tx, account()).await {
        Ok(password) => {
            tx.commit().await?;
            Ok(Some(password))
        }
        // Another instance created it first, so this one has no password to
        // show.
        Err(CreateError::NameTaken) => Ok(None),
        Err(CreateError::Database(error)) => Err(error),
    }
}

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
        // A first run that stopped before the superadmin existed is
        // recovered here.
        None => accounts::create(&mut tx, account()).await?,
    };
    tx.commit().await?;
    Ok(password)
}
