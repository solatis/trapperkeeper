//! Tenants and the roles each one is created with.

use std::fmt;

use sqlx::{AssertSqlSafe, FromRow};
use uuid::Uuid;

use super::{SYSTEM_TENANT_ID, Tx, on_tx, sql};

/// The `role_name` of the role a tenant's first admin holds.
pub const ADMIN_ROLE: &str = "admin";

/// The roles every created tenant holds, by name and title.
const ROLES: [(&str, &str); 3] = [
    (ADMIN_ROLE, "Admin"),
    ("operator", "Operator"),
    ("observer", "Observer"),
];

/// A live tenant, as listed and as found by name.
#[derive(FromRow, Debug)]
pub struct Tenant {
    #[sqlx(rename = "tenant_id")]
    pub id: String,
    #[sqlx(rename = "tenant_name")]
    pub name: String,
    #[sqlx(rename = "tenant_title")]
    pub title: String,
}

/// What a new tenant is made of.
pub struct NewTenant<'a> {
    /// Lowercase, as the name rule leaves it.
    pub name: &'a str,
    pub title: &'a str,
}

/// Why a tenant could not be created.
#[derive(Debug)]
pub enum CreateError {
    /// A live tenant has the name.
    NameTaken,
    Database(sqlx::Error),
}

impl fmt::Display for CreateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NameTaken => f.write_str("a live tenant has the name"),
            Self::Database(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for CreateError {}

impl From<sqlx::Error> for CreateError {
    fn from(error: sqlx::Error) -> Self {
        // The live-only unique index on the name is the one unique constraint
        // the inserts can break: every id is fresh, and the roles belong to
        // the new tenant.
        match error {
            sqlx::Error::Database(database) if database.is_unique_violation() => Self::NameTaken,
            error => Self::Database(error),
        }
    }
}

/// Creates a tenant with the roles admin, operator and observer, and returns
/// its id. Does not commit.
///
/// # Errors
///
/// `NameTaken` when a live tenant has the name; `Database` otherwise.
pub async fn create(tx: &mut Tx, new: NewTenant<'_>) -> Result<String, CreateError> {
    let tenant_id = Uuid::now_v7().to_string();
    let insert = sql(
        tx,
        "INSERT INTO tenants (tenant_id, tenant_name, tenant_title) VALUES ($1, $2, $3)",
    );
    on_tx!(tx, |conn| {
        sqlx::query(AssertSqlSafe(insert))
            .bind(&tenant_id)
            .bind(new.name)
            .bind(new.title)
            .execute(conn)
            .await
            .map(|_| ())
    })?;
    // Each tenant holds role rows of its own (doc/adr/0023).
    for (name, title) in ROLES {
        let insert = sql(
            tx,
            "INSERT INTO roles (role_id, tenant_id, role_name, role_title) VALUES ($1, $2, $3, $4)",
        );
        on_tx!(tx, |conn| {
            sqlx::query(AssertSqlSafe(insert))
                .bind(Uuid::now_v7().to_string())
                .bind(&tenant_id)
                .bind(name)
                .bind(title)
                .execute(conn)
                .await
                .map(|_| ())
        })?;
    }
    Ok(tenant_id)
}

/// The live tenants other than the system tenant, ordered by name.
///
/// # Errors
///
/// Fails when the database cannot be read.
pub async fn list(tx: &mut Tx) -> Result<Vec<Tenant>, sqlx::Error> {
    // `$tk` holds the superadmin and no customer, so it is left out.
    let select = sql(
        tx,
        "SELECT tenant_id, tenant_name, tenant_title FROM tenants \
         WHERE deleted_at IS NULL AND tenant_id <> $1 ORDER BY tenant_name",
    );
    on_tx!(tx, |conn| {
        sqlx::query_as(AssertSqlSafe(select))
            .bind(SYSTEM_TENANT_ID)
            .fetch_all(conn)
            .await
    })
}

/// The live tenant named `tenant_name`, or `None`.
///
/// # Errors
///
/// Fails when the database cannot be read.
pub async fn find(tx: &mut Tx, tenant_name: &str) -> Result<Option<Tenant>, sqlx::Error> {
    let select = sql(
        tx,
        "SELECT tenant_id, tenant_name, tenant_title FROM tenants \
         WHERE deleted_at IS NULL AND tenant_name = $1",
    );
    on_tx!(tx, |conn| {
        sqlx::query_as(AssertSqlSafe(select))
            .bind(tenant_name)
            .fetch_optional(conn)
            .await
    })
}
