//! Runs create-admin as a process against a model of the default tenant's
//! users, over generated sequences of runs. One property instead of a test per
//! outcome (doc/adr/0018).

mod common;

use std::collections::BTreeSet;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use argon2::Argon2;
use argon2::password_hash::PasswordVerifier;
use common::{TestDatabase, backends, migrate_database, on_pool, placeholders};
use proptest::prelude::*;
use sqlx::AssertSqlSafe;

/// The names a sequence draws from, so repeats occur.
const POOL: [&str; 4] = ["ada", "grace", "linus", "leon.m_1"];

/// The id of the default tenant, as the first migration seeds it.
const DEFAULT_TENANT: &str = "00000000-0000-7000-8000-000000000000";

/// The start of every stored hash: Argon2id at the parameters of
/// doc/adr/0022.
const HASH_PREFIX: &str = "$argon2id$v=19$m=65536,t=3,p=4$";

/// A name from the pool, written in random case, so the stored lowercase form
/// is checked.
fn name() -> impl Strategy<Value = String> {
    (proptest::sample::select(&POOL[..]), any::<u32>()).prop_map(|(name, mask)| {
        name.chars()
            .enumerate()
            .map(|(i, c)| {
                if (mask >> (i % 32)) & 1 == 1 {
                    c.to_ascii_uppercase()
                } else {
                    c
                }
            })
            .collect()
    })
}

/// One step of a generated sequence: a create-admin run, or a user deleted
/// through sqlx, because nothing deletes users yet.
#[derive(Debug, Clone)]
enum Step {
    Create(String),
    Delete(String),
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        3 => name().prop_map(Step::Create),
        1 => name().prop_map(Step::Delete),
    ]
}

/// Runs `create-admin name` on the database at `url` and returns its output.
fn create_admin(url: &str, name: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_trapperkeeper"))
        .args(["create-admin", name, "--database-url", url])
        .output()
        .expect("the binary runs")
}

/// What is stored for the live user named `name` in the default tenant: the
/// role name, the must-change flag and the password hash, or `None`.
fn stored(database: &TestDatabase, name: &str) -> Option<(String, bool, String)> {
    let select = placeholders(
        database.backend,
        "SELECT r.role_name, u.password_must_change, u.password_hash \
         FROM users u JOIN roles r ON r.tenant_id = u.tenant_id AND r.role_id = u.role_id \
         WHERE u.tenant_id = $1 AND u.user_name = $2 AND u.deleted_at IS NULL",
    );
    on_pool!(database, |pool| {
        sqlx::query_as(AssertSqlSafe(select))
            .bind(DEFAULT_TENANT)
            .bind(name)
            .fetch_optional(&pool)
            .await
            .expect("the user is read")
    })
}

/// Marks the live user named `name` in the default tenant deleted, as a
/// delete will once one exists. A name without a live user changes nothing.
fn delete(database: &TestDatabase, name: &str) {
    let update = placeholders(
        database.backend,
        "UPDATE users SET deleted_at = $1 \
         WHERE tenant_id = $2 AND user_name = $3 AND deleted_at IS NULL",
    );
    let now = i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("the clock is past 1970")
            .as_micros(),
    )
    .expect("a small number");
    on_pool!(database, |pool| {
        sqlx::query(AssertSqlSafe(update))
            .bind(now)
            .bind(DEFAULT_TENANT)
            .bind(name)
            .execute(&pool)
            .await
            .expect("the user is marked deleted");
    });
}

/// The last line of stdout, which is the printed password.
fn last_line(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .last()
        .unwrap_or_default()
        .to_owned()
}

proptest! {
    // 8 cases rather than proptest's 256: each run starts a process and hashes
    // with 64 MiB, on every backend. No regression file, so a failure leaves
    // nothing untracked in the tree; the failing seed is in the output.
    #![proptest_config(ProptestConfig {
        cases: 8,
        failure_persistence: None,
        .. ProptestConfig::default()
    })]

    /// On every backend, create-admin before migrate exits 1 and says to run
    /// migrate. After migrate, a run whose name no live user has exits 0,
    /// prints one 16-character alphanumeric password that matches the stored
    /// Argon2id hash, and stores a lowercase admin whose password must
    /// change. A run whose name a live user has exits 1 and says so. A
    /// deleted user frees its name.
    #[test]
    fn create_admin_follows_the_model(steps in proptest::collection::vec(step(), 0..=6)) {
        for backend in backends() {
            let database = TestDatabase::fresh(backend);

            let out = create_admin(&database.url, "first");
            prop_assert_eq!(out.status.code(), Some(1), "created before migrate on {}", backend);
            let stderr = String::from_utf8_lossy(&out.stderr);
            prop_assert!(stderr.contains("trapperkeeper migrate"), "{stderr:?} on {backend}");

            migrate_database(&database);
            // The model: the names of the live users.
            let mut live = BTreeSet::new();
            for step in &steps {
                match step {
                    Step::Delete(name) => {
                        let lower = name.to_lowercase();
                        delete(&database, &lower);
                        live.remove(&lower);
                        prop_assert!(stored(&database, &lower).is_none(), "{lower} is live on {backend}");
                    }
                    Step::Create(name) if live.contains(&name.to_lowercase()) => {
                        let out = create_admin(&database.url, name);
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        prop_assert_eq!(out.status.code(), Some(1), "{} created twice on {}", name, backend);
                        prop_assert!(stderr.contains("already exists"), "{stderr:?} on {backend}");
                    }
                    Step::Create(name) => {
                        let lower = name.to_lowercase();
                        let out = create_admin(&database.url, name);
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        prop_assert_eq!(out.status.code(), Some(0), "{} on {}: {}", name, backend, stderr);
                        let password = last_line(&out);
                        prop_assert!(
                            password.len() == 16 && password.chars().all(|c| c.is_ascii_alphanumeric()),
                            "{password:?} is not a 16-character alphanumeric password on {backend}"
                        );
                        let (role, must_change, hash) = stored(&database, &lower)
                            .unwrap_or_else(|| panic!("{name} is not stored on {backend}"));
                        prop_assert_eq!(role, "admin");
                        prop_assert!(must_change);
                        prop_assert!(hash.starts_with(HASH_PREFIX), "{hash:?} on {backend}");
                        // The parameters come from the PHC string, so the
                        // default hasher checks a hash made with ours.
                        prop_assert!(
                            Argon2::default().verify_password(password.as_bytes(), hash.as_str()).is_ok(),
                            "the printed password does not match the stored hash on {backend}"
                        );
                        live.insert(lower);
                    }
                }
            }
        }
    }
}
