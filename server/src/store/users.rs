//! Users and their roles in the relational store: creating and listing
//! users, finding one by name, by tenant name or by id, counting the live
//! admins, changing a role, deleting softly, revoking tokens and setting a
//! password. Every read ignores deleted users except `find_id_any`, which
//! also finds deleted users, so that the audit trail of a deleted user can
//! still be asked for by name.

use std::fmt;

use sqlx::{AssertSqlSafe, FromRow};
use uuid::Uuid;

use super::tenants::ADMIN_ROLE;
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
    /// Tells the client at login that the user is to replace the password,
    /// so that a client such as the UI asks for a new one. The server
    /// restricts nothing because of it.
    pub password_must_change: bool,
    /// The user's revocation cutoff in epoch microseconds in UTC. A token
    /// whose issue time lies before it is refused.
    pub tokens_valid_after: i64,
}

/// The columns of `User`, from users joined with the user's live role,
/// filtered to live users and live roles of the tenant bound as `$1`. Each
/// caller appends a condition whose placeholders start at `$2`, or an ORDER
/// BY. The macro does not join tenants, so it does not check that the tenant
/// is live. Its join on live roles leaves out a live user whose role is
/// deleted. A macro, because `concat!` takes only literals.
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
        match error {
            // The users table has two unique constraints, the primary key on
            // `user_id` and the live-only unique index on (tenant_id,
            // user_name). The id is a fresh version 7 UUID, so a unique
            // violation can only come from the name.
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
/// Returns `NameTaken` when a live user of the tenant has the name. Returns
/// `Database` for any other failure, including a role name that names no
/// live role of the tenant.
pub async fn create(tx: &mut Tx, new: NewUser<'_>) -> Result<(), CreateError> {
    let user_id = Uuid::now_v7().to_string();
    let now = now_micros();
    // The subselect looks up the role id from the tenant and the role name
    // among live roles, so callers never hold role ids (ADR-0023). When the
    // tenant holds no live role of that name, the subselect yields NULL and
    // the NOT NULL `role_id` refuses the insert.
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
            // Each placeholder number appears once and in ascending order,
            // because MariaDB's `?` binds by position, so a value used twice
            // gets two numbers and two binds (ADR-0019).
            .bind(new.tenant_id)
            .bind(new.role_name)
            .bind(new.password_hash)
            .bind(new.password_must_change)
            .bind(now)
            .bind(now)
            .execute(conn)
            .await
            // `on_tx` is a match whose arms run the body once per backend,
            // each with its own query result type, so the closure discards
            // the result to give every arm the type `()`.
            .map(|_| ())
    })?;
    Ok(())
}

/// The live users of `tenant_id` with their role names, ordered by name.
///
/// # Errors
///
/// Fails when the database cannot be read.
pub async fn list(tx: &mut Tx, tenant_id: &str) -> Result<Vec<User>, sqlx::Error> {
    let select = sql(tx, concat!(SELECT_USER!(), " ORDER BY u.user_name"));
    on_tx!(tx, |conn| {
        sqlx::query_as(AssertSqlSafe(select))
            .bind(tenant_id)
            .fetch_all(conn)
            .await
    })
}

/// How many live users of `tenant_id` hold the live role `admin`.
///
/// # Errors
///
/// Fails when the database cannot be read.
pub async fn count_admins(tx: &mut Tx, tenant_id: &str) -> Result<i64, sqlx::Error> {
    // The route that deletes a user calls this count first and refuses the
    // delete of a last admin. `delete` then needs no such condition in the
    // WHERE of its UPDATE, so zero rows affected by that UPDATE always means
    // a missing user.
    let select = sql(
        tx,
        "SELECT COUNT(*) FROM users u \
         JOIN roles r ON r.tenant_id = u.tenant_id AND r.role_id = u.role_id \
         WHERE u.tenant_id = $1 AND u.deleted_at IS NULL AND r.deleted_at IS NULL \
         AND r.role_name = $2",
    );
    on_tx!(tx, |conn| {
        sqlx::query_scalar(AssertSqlSafe(select))
            .bind(tenant_id)
            .bind(ADMIN_ROLE)
            .fetch_one(conn)
            .await
    })
}

/// Gives the live user `user_id` of `tenant_id` the tenant's live role named
/// `role_name`. The caller has checked that the tenant holds the role, so
/// that an unknown role is answered as the caller's mistake and not as a
/// database error. When no live user of the tenant has the id, it changes
/// nothing and returns `Ok`.
///
/// # Errors
///
/// Fails when the database cannot be written, and also when a live user
/// matches and the tenant holds no live role of that name.
pub async fn set_role(
    tx: &mut Tx,
    tenant_id: &str,
    user_id: &str,
    role_name: &str,
) -> Result<(), sqlx::Error> {
    // The subselect looks up the role id from the tenant and the role name
    // among live roles, so callers never hold role ids (ADR-0023). A role
    // name that the tenant does not hold yields NULL, which the NOT NULL
    // `role_id` refuses.
    let update = sql(
        tx,
        "UPDATE users SET role_id = \
         (SELECT role_id FROM roles WHERE tenant_id = $1 AND role_name = $2 AND deleted_at IS NULL) \
         WHERE tenant_id = $3 AND user_id = $4 AND deleted_at IS NULL",
    );
    on_tx!(tx, |conn| {
        sqlx::query(AssertSqlSafe(update))
            .bind(tenant_id)
            .bind(role_name)
            // Each placeholder number appears once and in ascending order,
            // because MariaDB's `?` binds by position (ADR-0019).
            .bind(tenant_id)
            .bind(user_id)
            .execute(conn)
            .await
            .map(|_| ())
    })
}

/// Marks the live user `user_id` of `tenant_id` deleted at `now`, a time in
/// epoch microseconds in UTC. Returns whether a row changed.
///
/// # Errors
///
/// Fails when the database cannot be written.
pub async fn delete(
    tx: &mut Tx,
    tenant_id: &str,
    user_id: &str,
    now: i64,
) -> Result<bool, sqlx::Error> {
    // The UPDATE sets `deleted_at` and keeps the row, because events, the
    // audit trail and reports keep naming the user (ADR-0024).
    let update = sql(
        tx,
        "UPDATE users SET deleted_at = $1 \
         WHERE tenant_id = $2 AND user_id = $3 AND deleted_at IS NULL",
    );
    let affected = on_tx!(tx, |conn| {
        sqlx::query(AssertSqlSafe(update))
            .bind(now)
            .bind(tenant_id)
            .bind(user_id)
            .execute(conn)
            .await
            .map(|done| done.rows_affected())
    })?;
    Ok(affected > 0)
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
    // The join on tenants filters `deleted_at` too, so the users of a deleted
    // tenant are not found (ADR-0024). The query does not use `SELECT_USER`,
    // because that macro binds the tenant id as `$1`, and this query knows
    // the tenant only by name.
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

/// The id of the user named `user_name` in `tenant_id`, deleted or not: the
/// live one when there is one, else the most recently created, or `None`.
///
/// # Errors
///
/// Fails when the database cannot be read.
pub async fn find_id_any(
    tx: &mut Tx,
    tenant_id: &str,
    user_name: &str,
) -> Result<Option<String>, sqlx::Error> {
    // Deleted users count, because the audit trail of a deleted user is
    // still asked for by name. A name can be used again after a delete
    // (ADR-0024), so the ORDER BY puts the live user first and then the most
    // recently created. Two deleted users of one name that were created in
    // the same microsecond tie, and the database then returns either.
    let select = sql(
        tx,
        "SELECT user_id FROM users WHERE tenant_id = $1 AND user_name = $2 \
         ORDER BY (deleted_at IS NULL) DESC, created_at DESC LIMIT 1",
    );
    on_tx!(tx, |conn| {
        sqlx::query_scalar(AssertSqlSafe(select))
            .bind(tenant_id)
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

/// Sets the user's `tokens_valid_after` to `now`, a time in epoch
/// microseconds in UTC, which revokes every token issued before it. When no
/// live user of the tenant has the id, it changes nothing and returns `Ok`.
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
    /// The new `password_must_change`.
    pub must_change: bool,
    /// The new `tokens_valid_after`.
    pub now: i64,
}

/// Replaces the user's password hash, sets the must-change flag, and sets
/// `tokens_valid_after` to `now`, which revokes every token issued before
/// the change (ADR-0025). When no live user of the tenant has the id, it
/// changes nothing and returns `Ok`.
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
