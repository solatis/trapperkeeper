//! Runs migrate and serve as processes against each backend the environment
//! names: what migrate builds, and which databases serve accepts. Two tests
//! cover the store, a migrate scenario and a property over generated
//! databases, because `doc/adr/0018` asks for few tests over a large surface.

mod common;

use std::collections::BTreeSet;
use std::process::{ExitStatus, Output};
use std::sync::OnceLock;

use common::{
    ADMIN, Backend, Launch, SYSTEM_TENANT, TestDatabase, backends, get, launch, migrate_database,
    on_each_backend, on_pool, placeholders, request, run_migrate,
};
use proptest::prelude::*;
use serde_json::{Value, json};
use sqlx::AssertSqlSafe;

/// The `count` of the line migrate logged after applying. Panics when migrate
/// failed or logged no such line.
fn applied_count(out: &Output) -> u64 {
    assert!(
        out.status.success(),
        "migrate failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("one JSON object per line"))
        .find(|line| line["fields"]["message"] == "migrations applied")
        .and_then(|line| line["fields"]["count"].as_u64())
        .expect("a migrations applied line with a count")
}

/// The passwords of the "superadmin created" lines among the JSON log
/// `lines`.
fn created_passwords<'a>(lines: impl IntoIterator<Item = &'a Value>) -> Vec<String> {
    lines
        .into_iter()
        .filter(|line| line["fields"]["message"] == "superadmin created")
        .map(|line| {
            line["fields"]["password"]
                .as_str()
                .expect("a password field")
                .to_owned()
        })
        .collect()
}

/// The JSON lines migrate wrote to stdout.
fn stdout_lines(out: &Output) -> Vec<Value> {
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|line| serde_json::from_str(line).expect("one JSON object per line"))
        .collect()
}

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

/// migrate builds a missing database with the system tenant, its superadmin
/// role, the superadmin and a signing salt, and logs the superadmin's
/// one-time password once. A second migrate applies nothing, keeps the salt
/// and logs no password. serve accepts the result.
#[test]
fn migrate_builds_a_database_that_serve_accepts() {
    // 1. migrate a missing database and read what it built
    // 2. migrate again
    // 3. start serve on the result
    on_each_backend(|backend| {
        let database = TestDatabase::fresh(backend);

        // 1. The first migrate is the one that creates the superadmin, so its
        //    stdout holds exactly one password event.
        let out = run_migrate(&database.url);
        let first = applied_count(&out);
        let passwords = created_passwords(&stdout_lines(&out));
        assert_eq!(passwords.len(), 1, "{passwords:?} on {backend}");
        assert!(generated(&passwords[0]), "{passwords:?} on {backend}");
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
        // Every migration the tracking table lists ran in this migrate.
        assert!(first > 0);
        assert_eq!(
            usize::try_from(first).expect("a small count"),
            tracked_versions(&database).len()
        );
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

        // 2. A second migrate changes nothing: the salt stays, because the
        //    keys derived from it would change, and the superadmin exists,
        //    so no second password is logged.
        let out = run_migrate(&database.url);
        assert_eq!(applied_count(&out), 0);
        assert_eq!(created_passwords(&stdout_lines(&out)), Vec::<String>::new());
        assert_eq!(salt(), first_salt);

        // 3.
        let Launch::Listening(server) = launch(&["--database-url", &database.url]) else {
            panic!("serve refused the database migrate built");
        };
        assert_eq!(get(server.addr, "/healthz").0, 200);
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
    Starts,
    /// Exits 1, and stderr contains each of these fragments.
    Refuses(Vec<String>),
}

/// The versions migrate applies, read once from the tracking table of a
/// freshly migrated SQLite database. Every backend has the same versions.
fn embedded_versions() -> &'static [i64] {
    static VERSIONS: OnceLock<Vec<i64>> = OnceLock::new();
    VERSIONS.get_or_init(|| {
        let database = TestDatabase::fresh(Backend::Sqlite);
        migrate_database(&database);
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

/// The versions as serve lists them in its message.
fn list(versions: &[i64]) -> String {
    versions
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// What serve must do for `state`: start; refuse naming the unknown, then
/// edited, then pending versions; or fail to apply the migrations over a
/// tracking table emptied beside the tables.
fn expected(state: &State) -> Outcome {
    // With no database, serve creates it and sets it up.
    let State::Migrated { rows, unknown } = state else {
        return Outcome::Starts;
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
    // pending. An emptied table reads as a new database, so serve applies
    // the first migration over the tables it already created, and sqlx
    // names the migration that failed.
    if unknown.is_empty() && missing.len() == rows.len() {
        Outcome::Refuses(vec!["while executing migration".to_owned()])
    } else if !unknown.is_empty() {
        Outcome::Refuses(vec![
            format!("migrations {}", list(&unknown)),
            "rebuild".to_owned(),
        ])
    } else if !edited.is_empty() {
        Outcome::Refuses(vec![
            format!("migrations {}", list(&edited)),
            "rebuild".to_owned(),
        ])
    } else if !missing.is_empty() {
        Outcome::Refuses(vec![
            format!("migrations {}", list(&missing)),
            "trapperkeeper migrate".to_owned(),
        ])
    } else {
        Outcome::Starts
    }
}

/// Builds `state` in `database`.
fn build(state: &State, database: &TestDatabase) {
    // An absent database is a URL at which nothing has been created.
    let State::Migrated { rows, unknown } = state else {
        return;
    };
    migrate_database(database);
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

/// Starts serve on the database at `url` and says what it did. A started
/// server is killed.
fn outcome(url: &str) -> (Option<ExitStatus>, String) {
    match launch(&["--database-url", url]) {
        Launch::Listening(_) => (None, String::new()),
        Launch::Exited(status, stderr) => (Some(status), stderr),
    }
}

proptest! {
    // 32 cases rather than proptest's 256: each case starts two processes per
    // backend. No regression file, so a failure leaves nothing untracked in the tree; the
    // failing seed is in the output.
    #![proptest_config(ProptestConfig {
        cases: 32,
        failure_persistence: None,
        .. ProptestConfig::default()
    })]

    /// On every backend, serve sets up a database that does not exist or has
    /// applied nothing, and starts on a database whose tracking table matches
    /// the binary's migrations. Otherwise it exits 1, names the differing
    /// versions and says what to do.
    #[test]
    fn serve_accepts_only_a_matching_database(state in database_state()) {
        // `build` makes the database the state describes, and `expected` is
        // the model of what serve must do with it.
        for backend in backends() {
            let database = TestDatabase::fresh(backend);
            build(&state, &database);
            let (status, stderr) = outcome(&database.url);
            match expected(&state) {
                Outcome::Starts => {
                    prop_assert!(status.is_none(), "serve refused on {backend}: {stderr}");
                }
                Outcome::Refuses(fragments) => {
                    prop_assert_eq!(
                        status.and_then(|s| s.code()), Some(1), "serve started on {}", backend
                    );
                    for fragment in fragments {
                        prop_assert!(
                            stderr.contains(&fragment),
                            "{fragment:?} not in {stderr:?} on {backend}"
                        );
                    }
                }
            }
            // A missing database is created by serve itself, which the exit
            // status alone does not show.
            if matches!(state, State::Absent) {
                prop_assert!(database.exists(), "serve did not create the database on {backend}");
            }
        }
    }
}

/// On every backend, the first serve on a missing database logs one
/// "superadmin created" line with a 16-character password before it listens,
/// and a login to `$tk` as `admin` with that password succeeds. A second
/// serve on the same database logs no such line.
#[test]
fn first_serve_sets_up_the_database() {
    // 1. start serve on a missing database
    // 2. log in with the password it logged
    // 3. start serve again on the same database
    on_each_backend(|backend| {
        // 1. `startup` holds the lines up to "listening", so the password
        //    event is logged before the server takes requests.
        let database = TestDatabase::fresh(backend);
        let Launch::Listening(server) = launch(&["--database-url", &database.url]) else {
            panic!("serve refused a missing database on {backend}");
        };
        let passwords = created_passwords(&server.startup);
        assert_eq!(passwords.len(), 1, "{passwords:?} on {backend}");
        assert!(generated(&passwords[0]), "{passwords:?} on {backend}");
        // 2. The in-process helpers do not apply to a process, so the login
        //    goes over HTTP to it.
        let credentials =
            json!({ "tenant": SYSTEM_TENANT, "name": ADMIN, "password": passwords[0] });
        let response = request(
            server.addr,
            "POST",
            "/api/v1/login",
            &[],
            Some(&credentials.to_string()),
        );
        assert_eq!(response.status, 200, "{} on {backend}", response.body);
        drop(server);

        // 3. The superadmin exists now, so this start logs no password.
        let Launch::Listening(server) = launch(&["--database-url", &database.url]) else {
            panic!("serve refused the database it set up on {backend}");
        };
        assert_eq!(created_passwords(&server.startup), Vec::<String>::new());
    });
}
