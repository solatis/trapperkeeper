//! Users and their roles in the relational store. Every read ignores deleted
//! rows.

use std::fmt;

use sqlx::{AssertSqlSafe, FromRow};
use uuid::Uuid;

use super::{Tx, now_micros, on_tx, sql};

/// A live user, as read with its role's name.
#[derive(FromRow, Debug, Clone)]
pub struct User {
    #[sqlx(rename = "user_id")]
    pub id: String,
    pub tenant_id: String,
    #[sqlx(rename = "user_name")]
    pub name: String,
    pub role_name: String,
    pub password_hash: String,
    pub password_must_change: bool,
    pub tokens_valid_after: i64,
}

/// The columns of `User`, joined with the user's live role and filtered to
/// the tenant, for each `find` to narrow. A macro, because `concat!` takes
/// only literals.
macro_rules! SELECT_USER {
    () => {
        "SELECT u.user_id, u.tenant_id, u.user_name, r.role_name, u.password_hash, \
         u.password_must_change, u.tokens_valid_after \
         FROM users u JOIN roles r ON r.tenant_id = u.tenant_id AND r.role_id = u.role_id \
         WHERE u.deleted_at IS NULL AND r.deleted_at IS NULL AND u.tenant_id = $1"
    };
}

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
    /// A live user of the tenant has the name.
    NameTaken,
    Database(sqlx::Error),
}

impl fmt::Display for CreateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NameTaken => f.write_str("a live user of the tenant has the name"),
            Self::Database(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for CreateError {}

impl From<sqlx::Error> for CreateError {
    fn from(error: sqlx::Error) -> Self {
        // The live-only unique index on the name is the only unique
        // constraint an insert can break: the id is fresh, and the role is
        // looked up.
        match error {
            sqlx::Error::Database(database) if database.is_unique_violation() => Self::NameTaken,
            error => Self::Database(error),
        }
    }
}

/// Creates a user. Its id is a new version 7 UUID, and `created_at` and
/// `tokens_valid_after` are now.
///
/// # Errors
///
/// `NameTaken` when a live user of the tenant has the name; `Database`
/// otherwise.
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

/// The live user named `user_name` in `tenant_id`, or `None`.
///
/// # Errors
///
/// Fails when the database cannot be read.
pub async fn find(
    tx: &mut Tx,
    tenant_id: &str,
    user_name: &str,
) -> Result<Option<User>, sqlx::Error> {
    let select = sql(tx, concat!(SELECT_USER!(), " AND u.user_name = $2"));
    on_tx!(tx, |conn| {
        sqlx::query_as(AssertSqlSafe(select))
            .bind(tenant_id)
            .bind(user_name)
            .fetch_optional(conn)
            .await
    })
}

/// The live user named `user_name` in the live tenant named `tenant_name`, or
/// `None`.
///
/// # Errors
///
/// Fails when the database cannot be read.
pub async fn find_in(
    tx: &mut Tx,
    tenant_name: &str,
    user_name: &str,
) -> Result<Option<User>, sqlx::Error> {
    // The join on tenants filters its deleted_at too, so the users of a
    // deleted tenant are not found (doc/adr/0024).
    let select = sql(
        tx,
        "SELECT u.user_id, u.tenant_id, u.user_name, r.role_name, u.password_hash, \
         u.password_must_change, u.tokens_valid_after \
         FROM users u \
         JOIN tenants t ON t.tenant_id = u.tenant_id \
         JOIN roles r ON r.tenant_id = u.tenant_id AND r.role_id = u.role_id \
         WHERE u.deleted_at IS NULL AND r.deleted_at IS NULL AND t.deleted_at IS NULL \
         AND t.tenant_name = $1 AND u.user_name = $2",
    );
    on_tx!(tx, |conn| {
        sqlx::query_as(AssertSqlSafe(select))
            .bind(tenant_name)
            .bind(user_name)
            .fetch_optional(conn)
            .await
    })
}

/// The live user with id `user_id` in `tenant_id`, or `None`.
///
/// # Errors
///
/// Fails when the database cannot be read.
pub async fn find_by_id(
    tx: &mut Tx,
    tenant_id: &str,
    user_id: &str,
) -> Result<Option<User>, sqlx::Error> {
    let select = sql(tx, concat!(SELECT_USER!(), " AND u.user_id = $2"));
    on_tx!(tx, |conn| {
        sqlx::query_as(AssertSqlSafe(select))
            .bind(tenant_id)
            .bind(user_id)
            .fetch_optional(conn)
            .await
    })
}

/// Sets the user's `tokens_valid_after` to `now`, which revokes every token
/// issued before it.
///
/// # Errors
///
/// Fails when the database cannot be written.
pub async fn revoke_tokens(
    tx: &mut Tx,
    tenant_id: &str,
    user_id: &str,
    now: i64,
) -> Result<(), sqlx::Error> {
    let update = sql(
        tx,
        "UPDATE users SET tokens_valid_after = $1 \
         WHERE tenant_id = $2 AND user_id = $3 AND deleted_at IS NULL",
    );
    on_tx!(tx, |conn| {
        sqlx::query(AssertSqlSafe(update))
            .bind(now)
            .bind(tenant_id)
            .bind(user_id)
            .execute(conn)
            .await
            .map(|_| ())
    })
}

/// What a password change writes.
pub struct PasswordSet<'a> {
    pub tenant_id: &'a str,
    pub user_id: &'a str,
    pub password_hash: &'a str,
    /// Whether the user must change the password at the next login.
    pub must_change: bool,
    /// The new `tokens_valid_after`.
    pub now: i64,
}

/// Replaces the user's password hash, sets the must-change flag, and sets
/// `tokens_valid_after` to `now`.
///
/// # Errors
///
/// Fails when the database cannot be written.
pub async fn set_password(tx: &mut Tx, change: PasswordSet<'_>) -> Result<(), sqlx::Error> {
    let update = sql(
        tx,
        "UPDATE users SET password_hash = $1, password_must_change = $2, tokens_valid_after = $3 \
         WHERE tenant_id = $4 AND user_id = $5 AND deleted_at IS NULL",
    );
    on_tx!(tx, |conn| {
        sqlx::query(AssertSqlSafe(update))
            .bind(change.password_hash)
            .bind(change.must_change)
            .bind(change.now)
            .bind(change.tenant_id)
            .bind(change.user_id)
            .execute(conn)
            .await
            .map(|_| ())
    })
}
