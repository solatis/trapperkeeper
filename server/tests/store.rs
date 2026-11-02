//! Runs migrate and serve as processes against SQLite files: what migrate
//! builds, and which databases serve accepts.

mod common;

use std::process::{Output, Stdio};

use common::{Scratch, get, run_migrate, spawn_server};
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

/// migrate builds a missing database with the default tenant; a second
/// migrate applies nothing; serve accepts the result.
#[test]
fn migrate_builds_a_database_that_serve_accepts() {
    let scratch = Scratch::create();
    let url = scratch.sqlite_url("store.db");

    let first = applied_count(&run_migrate(&url));
    // Read through sqlx on the file, because no endpoint shows tenants or
    // the tracking table.
    let (versions, tenants): (i64, Vec<(String, String)>) = with_pool(&url, async |pool| {
        let versions = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations")
            .fetch_one(pool)
            .await
            .expect("the tracking table is read");
        let tenants = sqlx::query_as("SELECT tenant_id, tenant_name FROM tenants")
            .fetch_all(pool)
            .await
            .expect("the tenants are read");
        (versions, tenants)
    });
    assert!(first > 0);
    assert_eq!(i64::try_from(first).expect("a small count"), versions);
    assert_eq!(
        tenants,
        [(
            "00000000-0000-7000-8000-000000000000".to_owned(),
            "default".to_owned()
        )]
    );

    assert_eq!(applied_count(&run_migrate(&url)), 0);

    let server = spawn_server(&["--database-url", &url], Stdio::null());
    assert_eq!(get(server.addr, "/healthz").0, 200);
}
