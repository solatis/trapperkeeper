//! Creating an admin, for the create-admin subcommand and for tests.

use std::fmt;

use crate::auth::password;
use crate::store::users::{self, ADMIN_ROLE, CreateError, NewUser};
use crate::store::{DEFAULT_TENANT_ID, Store};

/// Why an admin could not be created.
#[derive(Debug)]
pub enum AdminError {
    /// The name breaks the name rule.
    InvalidName(String),
    /// A live user of the default tenant has the name.
    NameTaken(String),
    Database(sqlx::Error),
}

impl fmt::Display for AdminError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName(name) => write!(
                f,
                "the name {name:?} is not 1 to 64 characters of a-z, 0-9, '.', '_' and '-'"
            ),
            Self::NameTaken(name) => write!(f, "a user named {name} already exists"),
            Self::Database(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for AdminError {}

impl From<sqlx::Error> for AdminError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

/// Creates an admin named `name` in the default tenant, with a generated
/// password that must be changed, commits, and returns the password.
///
/// # Errors
///
/// `InvalidName` when the name breaks the name rule; `NameTaken` when a live
/// user of the default tenant has it; `Database` otherwise.
pub async fn create(store: &Store, name: &str) -> Result<String, AdminError> {
    // Names are stored lowercase, so "Leon" and "leon" are one user
    // (doc/adr/0020).
    let name = name.to_lowercase();
    if !valid_name(&name) {
        return Err(AdminError::InvalidName(name));
    }
    let password = password::generate();
    let password_hash = password::hash(password.clone()).await;
    let mut tx = store.begin().await?;
    // create-admin may run again: a second admin is how a lost admin password
    // is recovered. Only a taken name is refused.
    users::create(
        &mut tx,
        NewUser {
            tenant_id: DEFAULT_TENANT_ID,
            user_name: &name,
            role_name: ADMIN_ROLE,
            password_hash: &password_hash,
            // The UI asks for a new password at the first login.
            password_must_change: true,
        },
    )
    .await
    .map_err(|error| match error {
        CreateError::NameTaken => AdminError::NameTaken(name.clone()),
        CreateError::Database(error) => AdminError::Database(error),
    })?;
    tx.commit().await?;
    Ok(password)
}

/// Whether `name` is 1 to 64 characters of a-z, 0-9, '.', '_' and '-'.
fn valid_name(name: &str) -> bool {
    // Letters, digits and this punctuation can be typed anywhere and fit in a
    // URL path as they are.
    (1..=64).contains(&name.len())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || ".-_".contains(c))
}
