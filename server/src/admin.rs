//! Creating an admin, for the create-admin subcommand and for tests.

use std::fmt;

use crate::auth::password;
use crate::store::users::{self, ADMIN_ROLE, CreateError, NewUser};
use crate::store::{DEFAULT_TENANT_ID, Store};

/// Why an admin could not be created.
#[derive(Debug)]
pub enum AdminError {
    Database(sqlx::Error),
}

impl fmt::Display for AdminError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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

impl From<CreateError> for AdminError {
    fn from(error: CreateError) -> Self {
        match error {
            CreateError::Database(error) => Self::Database(error),
        }
    }
}

/// Creates an admin named `name` in the default tenant, with a generated
/// password that must be changed, commits, and returns the password.
///
/// # Errors
///
/// `Database` when the store refuses the user or cannot be reached.
pub async fn create(store: &Store, name: &str) -> Result<String, AdminError> {
    // Names are stored lowercase, so "Leon" and "leon" are one user
    // (doc/adr/0020).
    let name = name.to_lowercase();
    let password = password::generate();
    let password_hash = password::hash(password.clone()).await;
    let mut tx = store.begin().await?;
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
    .await?;
    tx.commit().await?;
    Ok(password)
}
