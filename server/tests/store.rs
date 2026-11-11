//! What migrate builds, which databases serve accepts and what the first
//! serve sets up, through the library's setup functions, on each backend the
//! environment names: a scenario and a property over generated databases
//! (ADR-0018).

mod common;

use std::collections::BTreeSet;
use std::sync::OnceLock;

use common::{
    ADMIN, Backend, SYSTEM_TENANT, TestDatabase, backends, block_on, log_in, on_each_backend,
    on_pool, placeholders, serve_store, set_up_database,
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
    // 1. set up a missing database and read what it built
    // 2. set up again
    // 3. open the result as serve does
    on_each_backend(|backend| {
        let database = TestDatabase::fresh(backend);

        // 1. The first set-up is the one that creates the superadmin, so it is
        //    the one that returns a password.
        let (_, first) = block_on(setup::create(&database.url)).expect("the database is set up");
        let password = first.password.as_deref().expect("a password");
        assert!(generated(password), "{password:?} on {backend}");
        // Read through sqlx, because no endpoint shows tenants, roles or the
        // tracking table.
        let tenants: Vec<(String, String, String)> = on_pool!(database, |pool| {
            sqlx::query_as("SELECT tenant_id, tenant_name, tenant_title FROM tenants")
                .fetch_all(&pool)
                .await
                .expect("the tenants are read")
        });
        let roles: Vec<(String, String)> = on_pool!(database, |pool| {
            sqlx::query_as(
                "SELECT role_name, role_title FROM roles \
                 WHERE tenant_id = '00000000-0000-7000-8000-000000000000' \
                 ORDER BY role_id",
            )
            .fetch_all(&pool)
            .await
            .expect("the roles are read")
        });
        // Every migration the tracking table lists ran in this set-up.
        assert!(first.applied > 0);
        assert_eq!(first.applied, tracked_versions(&database).len());
        assert_eq!(
            tenants,
            [(
                "00000000-0000-7000-8000-000000000000".to_owned(),
                "$tk".to_owned(),
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
        // 16 random bytes as hex.
        let first_salt = salt();
        assert_eq!(first_salt.len(), 32);

        // 2. A second set-up changes nothing: the salt stays, because the keys
        //    derived from it would change, and the superadmin exists, so there
        //    is no second password.
        let (_, second) = block_on(setup::create(&database.url)).expect("the database is set up");
        assert_eq!(second.applied, 0);
        assert_eq!(second.password, None);
        assert_eq!(salt(), first_salt);

        // 3.
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
    /// A migrated database whose tracking table was then changed: one row
    /// state per embedded version, and versions the binary lacks.
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
    static VERSIONS: OnceLock<Vec<i64>> = OnceLock::new();
    VERSIONS.get_or_init(|| {
        let database = TestDatabase::fresh(Backend::Sqlite);
        set_up_database(&database);
        tracked_versions(&database)
    })
}

/// One generated database: none at all, or a migrated one in which each
/// embedded version is applied as is, missing from the table, or recorded
/// with another checksum, plus zero to two versions the binary lacks.
fn database_state() -> impl Strategy<Value = State> {
    // The versions come from the migrator's own list, so the property covers
    // every migration as it is added.
    let versions = embedded_versions();
    let last = *versions.last().expect("at least one migration");
    let row = prop_oneof![Just(Row::Applied), Just(Row::Missing), Just(Row::Edited)];
    let migrated = (
        proptest::collection::vec(row, versions.len()),
        proptest::collection::btree_set((1..1000_i64).prop_map(move |n| last + n), 0..=2),
    )
        .prop_map(|(rows, unknown)| State::Migrated {
            rows: versions.iter().copied().zip(rows).collect(),
            unknown,
        });
    prop_oneof![1 => Just(State::Absent), 7 => migrated]
}

/// What serve must do for `state`: set it up, open it, refuse it naming the
/// unknown, then the edited, then the pending versions, or fail to apply the
/// migrations over a tracking table emptied beside the tables.
fn expected(state: &State) -> Outcome {
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
    let unknown: Vec<i64> = unknown.iter().copied().collect();
    let (edited, missing) = (with(Row::Edited), with(Row::Missing));
    // The same precedence as `check`: empty, then unknown, edited and
    // pending. An emptied table reads as a new database, so the set-up
    // applies the first migration over the tables it already created, and
    // the migration fails.
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
                // One byte never equals a SHA-384 checksum.
                Row::Edited => {
                    sqlx::query(AssertSqlSafe(edit.clone()))
                        .bind(vec![0_u8])
                        .bind(version)
                        .execute(&pool)
                        .await
                }
            }
            .expect("the row is changed");
        }
        // The unknown versions lie above the last embedded one, so they never
        // collide with a real migration.
        for version in unknown {
            sqlx::query(AssertSqlSafe(insert.clone()))
                .bind(version)
                .bind(true)
                .bind(vec![0_u8])
                .execute(&pool)
                .await
                .expect("the row is inserted");
        }
    });
}

proptest! {
    // 32 cases rather than proptest's 256: each case creates a database on
    // every server backend. No regression file, so a failure leaves nothing
    // untracked in the tree; the failing seed is in the output.
    #![proptest_config(ProptestConfig {
        cases: 32,
        failure_persistence: None,
        .. ProptestConfig::default()
    })]

    /// On every backend, serve sets up a database that does not exist or has
    /// applied nothing, opens a database whose tracking table matches the
    /// binary's migrations, and otherwise refuses it naming the differing
    /// versions.
    #[test]
    fn serve_accepts_only_a_matching_database(state in database_state()) {
        // `build` makes the database the state describes, and `expected` is
        // the model of what serve must do with it.
        for backend in backends() {
            let database = TestDatabase::fresh(backend);
            build(&state, &database);
            let outcome = observed(block_on(setup::open_or_set_up(&database.url)));
            prop_assert_eq!(outcome, expected(&state), "on {}", backend);
            // A missing database is created by the set-up itself, which the
            // outcome alone does not show.
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
    // 1. open a missing database as serve does
    // 2. serve the router on the store it returned and log in
    // 3. open the same database again
    on_each_backend(|backend| {
        // 1.
        let database = TestDatabase::fresh(backend);
        let (store, set_up) = block_on(setup::open_or_set_up(&database.url))
            .expect("serve accepts a missing database");
        let password = set_up
            .and_then(|set_up| set_up.password)
            .expect("the first serve creates the superadmin");
        assert!(generated(&password), "{password:?} on {backend}");

        // 2. Only a login shows the password is the superadmin's.
        let server = serve_store(database, store, password.clone());
        log_in(&server, SYSTEM_TENANT, ADMIN, &password);

        // 3. The superadmin exists now, so this open sets nothing up.
        let (_, set_up) = block_on(setup::open_or_set_up(&server.database().url))
            .expect("serve accepts the database it set up");
        assert!(
            set_up.is_none(),
            "serve set up its own database again on {backend}"
        );
    });
}
