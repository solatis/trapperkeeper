//! What migrate builds, which databases serve accepts and what the first
//! serve sets up, through the library's setup functions. The file holds two
//! scenarios and one property over generated databases, as ADR-0018 prefers
//! a property to a case per state. Each runs on SQLite always, and on
//! Postgres and MariaDB when their admin URLs are set.

mod common;

use std::collections::BTreeSet;
use std::sync::OnceLock;

use common::{
    ADMIN, Backend, SYSTEM_TENANT, SYSTEM_TENANT_ID, TestDatabase, backends, block_on, log_in,
    on_each_backend, on_pool, placeholders, serve_store, set_up_database,
};
use proptest::prelude::*;
use sqlx::AssertSqlSafe;
use trapperkeeper_server::setup::{self, Error, SetUp};
use trapperkeeper_server::{Mismatch, Store};

/// Whether `password` is 16 ASCII letters and digits, as a generated one is.
fn generated(password: &str) -> bool {
    password.len() == 16 && password.chars().all(|c| c.is_ascii_alphanumeric())
}

/// The versions in the tracking table of `database`, in order.
fn tracked_versions(database: &TestDatabase) -> Vec<i64> {
    on_pool!(database, |pool| {
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&pool)
            .await
            .expect("the tracking table is read")
    })
}

/// migrate on a missing database creates the system tenant, its superadmin
/// role, the superadmin and a signing salt, and returns one generated
/// password. A second migrate applies nothing, keeps the salt and returns no
/// password. serve accepts the result.
#[test]
fn migrate_builds_a_database_that_serve_accepts() {
    // The test sets up a missing database twice as migrate does, then opens
    // it as serve does.
    //
    // 1. We set up a missing database and check what it built. Only this
    //    set-up creates the superadmin, so only it returns a password.
    // 2. We set it up again, which must apply nothing and keep the salt, from
    //    which the signing keys are derived.
    // 3. We open it as serve does, which must set nothing up.
    on_each_backend(|backend| {
        // 1. set up a missing database
        let database = TestDatabase::fresh(backend);
        let (_, first) = block_on(setup::create(&database.url)).expect("the database is set up");
        let password = first.password.as_deref().expect("a password");
        assert!(generated(password), "{password:?} on {backend}");
        // No route shows the system tenant `$tk`, its roles or the tracking
        // table, so the test reads them through sqlx.
        let tenants: Vec<(String, String, String)> = on_pool!(database, |pool| {
            sqlx::query_as("SELECT tenant_id, tenant_name, tenant_title FROM tenants")
                .fetch_all(&pool)
                .await
                .expect("the tenants are read")
        });
        let select = placeholders(
            backend,
            "SELECT role_name, role_title FROM roles WHERE tenant_id = $1 ORDER BY role_id",
        );
        // The rewritten statement is a `String` made from a literal, which
        // `AssertSqlSafe` lets through.
        let roles: Vec<(String, String)> = on_pool!(database, |pool| {
            sqlx::query_as(AssertSqlSafe(select))
                .bind(SYSTEM_TENANT_ID)
                .fetch_all(&pool)
                .await
                .expect("the roles are read")
        });
        // Every migration the tracking table lists ran in this set-up.
        assert!(first.applied > 0);
        assert_eq!(first.applied, tracked_versions(&database).len());
        // The first migrations seed the system tenant and its role.
        assert_eq!(
            tenants,
            [(
                SYSTEM_TENANT_ID.to_owned(),
                SYSTEM_TENANT.to_owned(),
                "TrapperKeeper".to_owned()
            )]
        );
        assert_eq!(roles, [("superadmin".to_owned(), "Superadmin".to_owned())]);

        let salt = || -> String {
            on_pool!(database, |pool| {
                sqlx::query_scalar("SELECT signing_salt FROM installation")
                    .fetch_one(&pool)
                    .await
                    .expect("the salt is read")
            })
        };
        let first_salt = salt();
        // The salt is 16 random bytes written as hex, so it has 32 characters.
        assert_eq!(first_salt.len(), 32);

        // 2. set up again
        let (_, second) = block_on(setup::create(&database.url)).expect("the database is set up");
        assert_eq!(second.applied, 0);
        assert_eq!(second.password, None);
        assert_eq!(salt(), first_salt);

        // 3. open as serve does
        let (_, set_up) = block_on(setup::open_or_set_up(&database.url))
            .expect("serve accepts the database migrate built");
        assert!(
            set_up.is_none(),
            "serve set up a database migrate built on {backend}"
        );
    });
}

/// What the tracking table says about one of the binary's migrations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Row {
    Applied,
    Missing,
    Edited,
}

/// One generated database.
#[derive(Debug, Clone)]
enum State {
    /// Nothing at the URL.
    Absent,
    /// A migrated database whose tracking table may then have been changed,
    /// with one row state per embedded version and zero to two versions that
    /// the binary lacks.
    Migrated {
        rows: Vec<(i64, Row)>,
        unknown: BTreeSet<i64>,
    },
}

/// What serve does with a database.
#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    /// Sets it up.
    SetsUp,
    /// Opens it as it is.
    Opens,
    /// Fails in a migration.
    FailsMigration,
    /// Refuses it, naming these versions the binary does not know.
    Unknown(Vec<i64>),
    /// Refuses it, naming these versions whose files differ.
    Edited(Vec<i64>),
    /// Refuses it, naming these versions it has not applied.
    Pending(Vec<i64>),
}

/// The versions migrate applies, read once from the tracking table of a
/// freshly migrated SQLite database. Every backend has the same versions.
fn embedded_versions() -> &'static [i64] {
    // The library does not export its migrations, so the versions come from
    // a migrated database, read once per process for all cases.
    static VERSIONS: OnceLock<Vec<i64>> = OnceLock::new();
    VERSIONS.get_or_init(|| {
        // SQLite runs in every test run, and each backend has every version.
        let database = TestDatabase::fresh(Backend::Sqlite);
        set_up_database(&database);
        tracked_versions(&database)
    })
}

/// One generated database: none at all, or a migrated one in which each
/// embedded version is applied as is, missing from the table, or recorded
/// with another checksum, plus zero to two versions the binary lacks.
fn database_state() -> impl Strategy<Value = State> {
    // A new migration enters the property without a change to the test.
    let versions = embedded_versions();
    let last = *versions.last().expect("at least one migration");
    let row = prop_oneof![Just(Row::Applied), Just(Row::Missing), Just(Row::Edited)];
    let migrated = (
        proptest::collection::vec(row, versions.len()),
        // Above the last real version, so no unknown one collides with it. The
        // range and the cap of two are a choice.
        proptest::collection::btree_set((1..1000_i64).prop_map(move |n| last + n), 0..=2),
    )
        .prop_map(|(rows, unknown)| State::Migrated {
            rows: versions.iter().copied().zip(rows).collect(),
            unknown,
        });
    // The weights are a choice.
    prop_oneof![1 => Just(State::Absent), 7 => migrated]
}

/// What serve must do with `state`. It sets up an absent database, fails in
/// the first migration when the tracking table is empty and the tables remain,
/// refuses a database with unknown, edited or pending versions in that order
/// and names them, and opens any other.
fn expected(state: &State) -> Outcome {
    // After the absent database, the cases follow the order in which
    // `migrate::check` reports a mismatch.

    // With no database, serve creates it and sets it up.
    let State::Migrated { rows, unknown } = state else {
        return Outcome::SetsUp;
    };
    let with = |r: Row| -> Vec<i64> {
        rows.iter()
            .filter(|(_, row)| *row == r)
            .map(|(v, _)| *v)
            .collect()
    };
    // Each list is in ascending order, as `check` lists versions.
    let unknown: Vec<i64> = unknown.iter().copied().collect();
    let (edited, missing) = (with(Row::Edited), with(Row::Missing));
    // An emptied tracking table reads as a new database, and the first
    // migration then fails over the tables that exist.
    if unknown.is_empty() && missing.len() == rows.len() {
        Outcome::FailsMigration
    } else if !unknown.is_empty() {
        Outcome::Unknown(unknown)
    } else if !edited.is_empty() {
        Outcome::Edited(edited)
    } else if !missing.is_empty() {
        Outcome::Pending(missing)
    } else {
        Outcome::Opens
    }
}

/// What serve did, read from the result of `open_or_set_up`. Panics on an
/// error the property never builds, such as a database that does not open.
fn observed(result: Result<(Store, Option<SetUp>), Error>) -> Outcome {
    match result {
        Ok((_, Some(_))) => Outcome::SetsUp,
        Ok((_, None)) => Outcome::Opens,
        Err(Error::Migrate(_)) => Outcome::FailsMigration,
        Err(Error::Mismatch(Mismatch::Unknown(versions))) => Outcome::Unknown(versions),
        Err(Error::Mismatch(Mismatch::Edited(versions))) => Outcome::Edited(versions),
        Err(Error::Mismatch(Mismatch::Pending(versions))) => Outcome::Pending(versions),
        Err(error) => panic!("{error}"),
    }
}

/// Builds `state` in `database`.
fn build(state: &State, database: &TestDatabase) {
    // An absent database is a URL at which nothing has been created.
    let State::Migrated { rows, unknown } = state else {
        return;
    };
    set_up_database(database);
    let delete = placeholders(
        database.backend,
        "DELETE FROM _sqlx_migrations WHERE version = $1",
    );
    let edit = placeholders(
        database.backend,
        "UPDATE _sqlx_migrations SET checksum = $1 WHERE version = $2",
    );
    let insert = placeholders(
        database.backend,
        "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) \
         VALUES ($1, 'unknown', $2, $3, 0)",
    );
    // The statements are literals with rewritten placeholders, so no outside
    // text reaches the SQL that `AssertSqlSafe` lets through.
    on_pool!(database, |pool| {
        for (version, row) in rows {
            match row {
                Row::Applied => continue,
                Row::Missing => {
                    sqlx::query(AssertSqlSafe(delete.clone()))
                        .bind(version)
                        .execute(&pool)
                        .await
                }
                Row::Edited => {
                    sqlx::query(AssertSqlSafe(edit.clone()))
                        // One byte never equals a SHA-384 checksum.
                        .bind(vec![0_u8])
                        .bind(version)
                        .execute(&pool)
                        .await
                }
            }
            .expect("the row is changed");
        }
        for version in unknown {
            sqlx::query(AssertSqlSafe(insert.clone()))
                .bind(version)
                // A failed row would be a dirty version, which the model
                // leaves out.
                .bind(true)
                .bind(vec![0_u8])
                .execute(&pool)
                .await
                .expect("the row is inserted");
        }
    });
}

proptest! {
    #![proptest_config(ProptestConfig {
        // 32 cases, because each creates a database on every backend.
        cases: 32,
        // No regression file is left in the tree. The panic message shows the
        // minimal failing input.
        failure_persistence: None,
        .. ProptestConfig::default()
    })]

    /// On every backend, serve sets up a database that does not exist, opens
    /// one whose tracking table matches the binary's migrations, fails in the
    /// first migration when the tracking table was emptied and the tables
    /// remain, and refuses any other, naming the versions that differ.
    #[test]
    fn serve_accepts_only_a_matching_database(state in database_state()) {
        for backend in backends() {
            let database = TestDatabase::fresh(backend);
            build(&state, &database);
            let outcome = observed(block_on(setup::open_or_set_up(&database.url)));
            prop_assert_eq!(outcome, expected(&state), "on {}", backend);
            // `SetsUp` does not show that serve created the database.
            if matches!(state, State::Absent) {
                prop_assert!(database.exists(), "serve did not create the database on {backend}");
            }
        }
    }
}

/// On every backend, the first serve on a missing database creates the
/// superadmin with one 16-character password, a login to `$tk` as `admin`
/// with it succeeds on the router served on that store, and a second serve on
/// the same database sets nothing up.
#[test]
fn first_serve_sets_up_the_database() {
    // The test opens a missing database as serve does, logs in, and opens it
    // again.
    //
    // 1. We open a missing database, which must set it up with a generated
    //    password.
    // 2. We serve it and log in, which shows the password is the superadmin's.
    // 3. We open it again, which must set nothing up, because its tracking
    //    table now matches the binary.
    on_each_backend(|backend| {
        // 1. open a missing database
        let database = TestDatabase::fresh(backend);
        let (store, set_up) = block_on(setup::open_or_set_up(&database.url))
            .expect("serve accepts a missing database");
        let password = set_up
            .and_then(|set_up| set_up.password)
            .expect("the first serve creates the superadmin");
        assert!(generated(&password), "{password:?} on {backend}");

        // 2. serve and log in
        let server = serve_store(database, store, password.clone());
        log_in(&server, SYSTEM_TENANT, ADMIN, &password);

        // 3. open again
        let (_, set_up) = block_on(setup::open_or_set_up(&server.database().url))
            .expect("serve accepts the database it set up");
        assert!(
            set_up.is_none(),
            "serve set up its own database again on {backend}"
        );
    });
}
