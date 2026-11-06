//! Creating users and resetting their passwords. Every user, the superadmin
//! too, gets a generated password that must be changed at the first login.

use crate::auth::password;
use crate::store::Tx;
use crate::store::users::{self, CreateError, NewUser};

/// Who a new user is.
pub struct NewAccount<'a> {
    pub tenant_id: &'a str,
    /// Stored lowercase.
    pub name: &'a str,
    /// One of the tenant's live roles.
    pub role_name: &'a str,
}

/// Creates the user with a generated password that must be changed, and
/// returns the password. Does not commit.
///
/// # Errors
///
/// `NameTaken` when a live user of the tenant has the name; `Database`
/// otherwise.
pub async fn create(tx: &mut Tx, new: NewAccount<'_>) -> Result<String, CreateError> {
    let password = password::generate();
    let password_hash = password::hash(password.clone()).await;
    users::create(
        tx,
        NewUser {
            tenant_id: new.tenant_id,
            // Names are stored lowercase, so "Leon" and "leon" are one user
            // (doc/adr/0020).
            user_name: &new.name.to_lowercase(),
            role_name: new.role_name,
            password_hash: &password_hash,
            // The person who receives the password replaces it at the first
            // login.
            password_must_change: true,
        },
    )
    .await?;
    Ok(password)
}
