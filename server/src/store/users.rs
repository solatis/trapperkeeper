//! Users and their roles in the relational store. Every read ignores deleted
//! rows.

use std::fmt;

use sqlx::AssertSqlSafe;
use uuid::Uuid;

use super::{Tx, now_micros, on_tx, sql};

/// The `role_name` of the admin role the migration seeds for every tenant.
pub const ADMIN_ROLE: &str = "admin";

/// What a new user is made of. `role_name` names one of the tenant's live
/// roles.
pub struct NewUser<'a> {
    pub tenant_id: &'a str,
    pub user_name: &'a str,
    pub role_name: &'a str,
    pub password_hash: &'a str,
    pub password_must_change: bool,
}

/// Why a user could not be created.
#[derive(Debug)]
pub enum CreateError {
    Database(sqlx::Error),
}

impl fmt::Display for CreateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for CreateError {}

impl From<sqlx::Error> for CreateError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

/// Creates a user. Its id is a new version 7 UUID, and `created_at` and
/// `tokens_valid_after` are now.
///
/// # Errors
///
/// `Database` when the database refuses the row or cannot be reached.
pub async fn create(tx: &mut Tx, new: NewUser<'_>) -> Result<(), CreateError> {
    let user_id = Uuid::now_v7().to_string();
    // tokens_valid_after starts at created_at, so no token can predate the
    // user.
    let now = now_micros();
    // The role id comes from a subselect on the tenant and the role name,
    // filtered to live roles, so callers never hold role ids.
    let insert = sql(
        tx,
        "INSERT INTO users (user_id, tenant_id, user_name, role_id, password_hash, \
         password_must_change, tokens_valid_after, created_at) \
         VALUES ($1, $2, $3, \
         (SELECT role_id FROM roles WHERE tenant_id = $4 AND role_name = $5 AND deleted_at IS NULL), \
         $6, $7, $8, $9)",
    );
    on_tx!(tx, |conn| {
        sqlx::query(AssertSqlSafe(insert))
            .bind(&user_id)
            .bind(new.tenant_id)
            .bind(new.user_name)
            .bind(new.tenant_id)
            .bind(new.role_name)
            .bind(new.password_hash)
            .bind(new.password_must_change)
            .bind(now)
            .bind(now)
            .execute(conn)
            .await
            // Each backend has a result type of its own; nothing in it is
            // needed.
            .map(|_| ())
    })?;
    Ok(())
}
