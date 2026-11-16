//! Creating users and resetting their passwords. Every user, the superadmin
//! too, gets a generated password and the must-change flag, which tells the
//! UI to ask for a new password at the next login and restricts no route.

use crate::auth::password;
use crate::store::users::{self, CreateError, NewUser, PasswordSet};
use crate::store::{Tx, now_micros};

/// Who a new user is.
pub struct NewAccount<'a> {
    pub tenant_id: &'a str,
    /// `create` lowercases the name before it stores it.
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
            // Two names that differ only in case name one user (ADR-0020).
            user_name: &new.name.to_lowercase(),
            role_name: new.role_name,
            password_hash: &password_hash,
            // The flag only tells the UI to ask for a new password.
            password_must_change: true,
        },
    )
    .await?;
    Ok(password)
}

/// Gives the user a new generated password that must be changed, revokes its
/// tokens, and returns the password. Does not commit. A `user_id` that names
/// no live user of the tenant changes no row, and the function still returns
/// a password.
///
/// # Errors
///
/// Fails when the database cannot be written.
pub async fn reset(tx: &mut Tx, tenant_id: &str, user_id: &str) -> Result<String, sqlx::Error> {
    let password = password::generate();
    let password_hash = password::hash(password.clone()).await;
    users::set_password(
        tx,
        PasswordSet {
            tenant_id,
            user_id,
            password_hash: &password_hash,
            must_change: true,
            // This revokes every token issued before the reset.
            now: now_micros(),
        },
    )
    .await?;
    Ok(password)
}
