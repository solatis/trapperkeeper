//! Runs reset-superadmin as a process on each backend: what it refuses before
//! the database is set up, what it creates on a migrated database without a
//! superadmin, and what a second run changes.

mod common;

use std::process::{Command, Output};

use argon2::Argon2;
use argon2::password_hash::PasswordVerifier;
use common::{ADMIN, SYSTEM_TENANT_ID, TestDatabase, migrate_database, on_each_backend, on_pool};
use sqlx::AssertSqlSafe;

/// The start of every stored hash: Argon2id at the parameters of
/// doc/adr/0022.
const HASH_PREFIX: &str = "$argon2id$v=19$m=65536,t=3,p=4$";

/// Runs `reset-superadmin` on the database at `url` and returns its output.
fn reset_superadmin(url: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_trapperkeeper"))
        .args(["reset-superadmin", "--database-url", url])
        .output()
        .expect("the binary runs")
}

/// What is stored for the live superadmin: the role name, the must-change
/// flag, the password hash and `tokens_valid_after`, or `None`.
fn stored(database: &TestDatabase) -> Option<(String, bool, String, i64)> {
    let select = common::placeholders(
        database.backend,
        "SELECT r.role_name, u.password_must_change, u.password_hash, u.tokens_valid_after \
         FROM users u JOIN roles r ON r.tenant_id = u.tenant_id AND r.role_id = u.role_id \
         WHERE u.tenant_id = $1 AND u.user_name = $2 AND u.deleted_at IS NULL",
    );
    on_pool!(database, |pool| {
        sqlx::query_as(AssertSqlSafe(select))
            .bind(SYSTEM_TENANT_ID)
            .bind(ADMIN)
            .fetch_optional(&pool)
            .await
            .expect("the superadmin is read")
    })
}

/// Marks the superadmin deleted, as a delete will once one exists.
fn delete(database: &TestDatabase) {
    let update = common::placeholders(
        database.backend,
        "UPDATE users SET deleted_at = 1 WHERE tenant_id = $1 AND user_name = $2",
    );
    on_pool!(database, |pool| {
        sqlx::query(AssertSqlSafe(update))
            .bind(SYSTEM_TENANT_ID)
            .bind(ADMIN)
            .execute(&pool)
            .await
            .expect("the superadmin is marked deleted");
    });
}

/// Runs reset-superadmin, checks that it printed a one-time password that the
/// stored superadmin holds, and returns the password and `tokens_valid_after`.
fn reset_and_check(database: &TestDatabase) -> (String, i64) {
    let out = reset_superadmin(&database.url);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{stderr}");
    // The log lines share stdout, so the password is the last line.
    let stdout = String::from_utf8_lossy(&out.stdout);
    let password = stdout.lines().last().unwrap_or_default().to_owned();
    assert!(
        password.len() == 16 && password.chars().all(|c| c.is_ascii_alphanumeric()),
        "{stdout:?} is not a 16-character alphanumeric password"
    );
    let (role, must_change, hash, valid_after) = stored(database).expect("a live superadmin");
    assert_eq!(role, "superadmin");
    assert!(must_change);
    assert!(hash.starts_with(HASH_PREFIX), "{hash:?}");
    // The parameters come from the PHC string, so the default hasher checks a
    // hash made with ours.
    assert!(
        Argon2::default()
            .verify_password(password.as_bytes(), hash.as_str())
            .is_ok(),
        "the printed password does not match the stored hash"
    );
    (password, valid_after)
}

/// On every backend, reset-superadmin on a missing database exits 1 and says
/// how to set it up. After migrate, which creates the superadmin, a run
/// prints a 16-character alphanumeric password that matches the stored
/// Argon2id hash, sets the must-change flag and moves `tokens_valid_after`
/// forward. A second run prints another password. With the superadmin
/// deleted through sqlx, a run creates it again.
#[test]
fn reset_superadmin_sets_a_new_one_time_password() {
    // A scenario, because no input varies (doc/adr/0018).
    on_each_backend(|backend| {
        let database = TestDatabase::fresh(backend);
        let out = reset_superadmin(&database.url);
        assert_eq!(
            out.status.code(),
            Some(1),
            "reset before migrate on {backend}"
        );
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("trapperkeeper migrate"),
            "{stderr:?} on {backend}"
        );

        migrate_database(&database);
        let (_, _, created_hash, created_valid_after) =
            stored(&database).expect("migrate created the superadmin");
        let (first, first_valid_after) = reset_and_check(&database);
        assert!(
            first_valid_after > created_valid_after,
            "the cutoff stayed on {backend}"
        );
        let (_, _, first_hash, _) = stored(&database).expect("a live superadmin");
        assert_ne!(first_hash, created_hash);
        let (second, second_valid_after) = reset_and_check(&database);
        assert_ne!(first, second);
        assert!(
            second_valid_after > first_valid_after,
            "the cutoff stayed on {backend}"
        );

        delete(&database);
        reset_and_check(&database);
    });
}
