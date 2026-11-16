//! Tenants and the roles each one is created with: creating, listing and
//! finding a tenant, and whether a tenant holds a role.

use std::fmt;

use sqlx::{AssertSqlSafe, FromRow};
use uuid::Uuid;

use super::{SYSTEM_TENANT_ID, Tx, on_tx, sql};

/// The `role_name` of the admin role that `create` gives every tenant it
/// makes. The first user of a created tenant holds it, and the rule that a
/// tenant keeps at least one live admin counts the users who hold it.
pub const ADMIN_ROLE: &str = "admin";

/// The roles every created tenant holds, each as a `role_name` and a
/// `role_title`. ADR-0023 decides that a tenant the superadmin creates gets
/// the roles admin, operator and observer.
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
    /// The name as `names::tenant_name` returns it. It must be lowercase,
    /// because the tenants table refuses any other name with a CHECK, and
    /// `create` then fails with `Database` and not with `NameTaken`.
    pub name: &'a str,
    /// A display name that keeps its case. The MariaDB column holds at most
    /// 255 characters, and `create` does not check the length, so the caller
    /// does.
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
        match error {
            // The only unique constraint that the inserts of `create` can
            // violate is the unique index on the names of live tenants.
            // Every tenant id and role id is a fresh version 7 UUID, so the
            // primary keys and the unique key on (tenant_id, role_id) cannot
            // collide. The three role names of `ROLES` are distinct and
            // belong to the new tenant, so the unique index on live role
            // names cannot be violated either.
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
/// Returns `NameTaken` when a live tenant has the name, and `Database` when
/// the store fails in any other way.
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
            // Each backend returns a query result type of its own, and the
            // arms that `on_tx!` expands to must yield one type. `create`
            // needs nothing from the result, so the closure drops it.
            .map(|_| ())
    })?;
    // Roles are rows per tenant, because nearly every table carries a tenant
    // id, and so that a tenant may hold roles of its own (ADR-0023).
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
    // The query leaves out `$tk`, because `$tk` holds only the superadmin and
    // is not a customer (ADR-0028).
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

/// The live tenant named `tenant_name`, or `None`. The function compares
/// `tenant_name` byte for byte with stored names, which are always lowercase,
/// so a caller passes a lowercased name or gets `None`. Unlike `list`, `find`
/// also returns the system tenant `$tk`, so a caller that must hide it
/// filters it out.
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
            // The unique index on the names of live tenants lets at most one
            // live row match.
            .fetch_optional(conn)
            .await
    })
}

/// Whether `tenant_id` holds a live role named `role_name`. The function
/// compares `role_name` byte for byte with stored role names, which are
/// always lowercase, so a name in another case matches no role.
///
/// # Errors
///
/// Fails when the database cannot be read.
pub async fn role_exists(
    tx: &mut Tx,
    tenant_id: &str,
    role_name: &str,
) -> Result<bool, sqlx::Error> {
    let select = sql(
        tx,
        "SELECT COUNT(*) FROM roles \
         WHERE tenant_id = $1 AND role_name = $2 AND deleted_at IS NULL",
    );
    let count: i64 = on_tx!(tx, |conn| {
        sqlx::query_scalar(AssertSqlSafe(select))
            .bind(tenant_id)
            .bind(role_name)
            .fetch_one(conn)
            .await
    })?;
    Ok(count > 0)
}
