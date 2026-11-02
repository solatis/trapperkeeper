//! Runs migrate and serve as processes against SQLite files: what migrate
//! builds, and which databases serve accepts. Two tests cover the store, a
//! migrate scenario and a property over generated databases, because
//! `doc/adr/0018` asks for few tests over a large surface.

mod common;

use std::collections::BTreeSet;
use std::process::{ExitStatus, Output, Stdio};
use std::sync::OnceLock;

use common::{Launch, Scratch, get, launch, migrated_database, run_migrate};
use proptest::prelude::*;
use serde_json::Value;
use sqlx::sqlite::SqlitePool;

/// Runs `query` on a pool for the SQLite database at `url`, on a runtime of its
/// own, because the tests are synchronous and sqlx is not.
fn with_pool<T>(url: &str, query: impl AsyncFnOnce(&SqlitePool) -> T) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime")
        .block_on(async {
            let pool = SqlitePool::connect(url).await.expect("the database opens");
            let result = query(&pool).await;
            pool.close().await;
            result
        })
}

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

/// The versions in the tracking table of the database at `url`, in order.
fn tracked_versions(url: &str) -> Vec<i64> {
    with_pool(url, async |pool| {
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
            .fetch_all(pool)
            .await
            .expect("the tracking table is read")
    })
}

/// migrate builds a missing database with the default tenant; a second
/// migrate applies nothing; serve accepts the result.
#[test]
fn migrate_builds_a_database_that_serve_accepts() {
    let scratch = Scratch::create();
    let url = scratch.sqlite_url("store.db");

    let first = applied_count(&run_migrate(&url));
    // Read through sqlx on the file, because no endpoint shows tenants or
    // the tracking table.
    let tenants: Vec<(String, String)> = with_pool(&url, async |pool| {
        sqlx::query_as("SELECT tenant_id, tenant_name FROM tenants")
            .fetch_all(pool)
            .await
            .expect("the tenants are read")
    });
    assert!(first > 0);
    assert_eq!(
        usize::try_from(first).expect("a small count"),
        tracked_versions(&url).len()
    );
    assert_eq!(
        tenants,
        [(
            "00000000-0000-7000-8000-000000000000".to_owned(),
            "default".to_owned()
        )]
    );

    assert_eq!(applied_count(&run_migrate(&url)), 0);

    let Launch::Listening(server) = launch(&["--database-url", &url], Stdio::null()) else {
        panic!("serve refused the database migrate built");
    };
    assert_eq!(get(server.addr, "/healthz").0, 200);
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
    /// No file at the URL.
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
/// freshly migrated database.
fn embedded_versions() -> &'static [i64] {
    static VERSIONS: OnceLock<Vec<i64>> = OnceLock::new();
    VERSIONS.get_or_init(|| {
        let scratch = Scratch::create();
        tracked_versions(&migrated_database(&scratch))
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
/// edited, then pending versions; or, with no database, refuse saying to run
/// migrate.
fn expected(state: &State) -> Outcome {
    // With no database, serve fails to open it, and its message says that
    // migrate creates it.
    let State::Migrated { rows, unknown } = state else {
        return Outcome::Refuses(vec!["trapperkeeper migrate".to_owned()]);
    };
    let with = |r: Row| -> Vec<i64> {
        rows.iter()
            .filter(|(_, row)| *row == r)
            .map(|(v, _)| *v)
            .collect()
    };
    let unknown: Vec<i64> = unknown.iter().copied().collect();
    let (edited, missing) = (with(Row::Edited), with(Row::Missing));
    // The same precedence as `check`: unknown, then edited, then pending.
    if !unknown.is_empty() {
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

/// Builds `state` at a URL in `scratch` and returns the URL.
fn build(state: &State, scratch: &Scratch) -> String {
    // An absent database is a URL with no file behind it.
    let State::Migrated { rows, unknown } = state else {
        return scratch.sqlite_url("store.db");
    };
    let url = migrated_database(scratch);
    with_pool(&url, async |pool| {
        for (version, row) in rows {
            // `X'00'` is one byte, so it never equals a SHA-384 checksum.
            let statement = match row {
                Row::Applied => continue,
                Row::Missing => "DELETE FROM _sqlx_migrations WHERE version = $1",
                Row::Edited => "UPDATE _sqlx_migrations SET checksum = X'00' WHERE version = $1",
            };
            sqlx::query(statement)
                .bind(version)
                .execute(pool)
                .await
                .expect("the row is changed");
        }
        // The unknown versions lie above the last embedded one, so they never
        // collide with a real migration.
        for version in unknown {
            sqlx::query(
                "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) \
                 VALUES ($1, 'unknown', true, X'00', 0)",
            )
            .bind(version)
            .execute(pool)
            .await
            .expect("the row is inserted");
        }
    });
    url
}

/// Starts serve on the database at `url` and says what it did. A started
/// server is killed.
fn outcome(url: &str) -> (Option<ExitStatus>, String) {
    match launch(&["--database-url", url], Stdio::null()) {
        Launch::Listening(_) => (None, String::new()),
        Launch::Exited(status, stderr) => (Some(status), stderr),
    }
}

proptest! {
    // 32 cases rather than proptest's 256: each case starts two processes. No
    // regression file, so a failure leaves nothing untracked in the tree; the
    // failing seed is in the output.
    #![proptest_config(ProptestConfig {
        cases: 32,
        failure_persistence: None,
        .. ProptestConfig::default()
    })]

    /// serve starts on a database when its tracking table matches the
    /// binary's migrations. Otherwise it exits 1, names the differing
    /// versions and says what to do, and it creates no missing database.
    #[test]
    fn serve_accepts_only_a_matching_database(state in database_state()) {
        let scratch = Scratch::create();
        let url = build(&state, &scratch);
        let (status, stderr) = outcome(&url);
        match expected(&state) {
            Outcome::Starts => prop_assert!(status.is_none(), "serve refused: {stderr}"),
            Outcome::Refuses(fragments) => {
                prop_assert_eq!(status.and_then(|s| s.code()), Some(1), "serve started");
                for fragment in fragments {
                    prop_assert!(stderr.contains(&fragment), "{fragment:?} not in {stderr:?}");
                }
            }
        }
        if matches!(state, State::Absent) {
            prop_assert!(!scratch.path.join("store.db").exists(), "serve created the database");
        }
    }
}
