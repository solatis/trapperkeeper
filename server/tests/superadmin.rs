//! reset-superadmin through the library, on each backend.

mod common;

use argon2::Argon2;
use argon2::password_hash::PasswordVerifier;
use common::{
    ADMIN, SYSTEM_TENANT_ID, TestDatabase, block_on, on_each_backend, on_pool, set_up_database,
};
use sqlx::AssertSqlSafe;
use trapperkeeper_server::setup::{self, Error};
use trapperkeeper_server::{Store, reset_superadmin};

/// The start of every stored hash. It names Argon2id version 19 with 64 MiB
/// of memory (`m=65536` KiB), 3 passes and 4 lanes, the parameters of
/// ADR-0022.
const HASH_PREFIX: &str = "$argon2id$v=19$m=65536,t=3,p=4$";

/// The role name, the must-change flag, the password hash and
/// `tokens_valid_after` of the live superadmin, or `None` when no live
/// superadmin exists.
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

/// Marks the superadmin of `$tk` deleted, as a first run leaves `$tk` when it
/// stops before it creates the superadmin. The helper writes the row itself,
/// because no route reaches `$tk`.
fn delete(database: &TestDatabase) {
    // Any `deleted_at` that is not NULL marks the row deleted, so the test
    // needs no real time.
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

/// Runs the reset on `store`, checks that the returned one-time password is
/// the one the stored superadmin holds, and returns it with
/// `tokens_valid_after`.
fn reset_and_check(store: &Store, database: &TestDatabase) -> (String, i64) {
    let password = block_on(reset_superadmin(store)).expect("the reset succeeds");
    // A generated password is 16 random ASCII letters and digits (ADR-0022).
    assert!(
        password.len() == 16 && password.chars().all(|c| c.is_ascii_alphanumeric()),
        "{password:?} is not a 16-character alphanumeric password"
    );
    let (role, must_change, hash, valid_after) = stored(database).expect("a live superadmin");
    assert_eq!(role, "superadmin");
    assert!(must_change);
    assert!(hash.starts_with(HASH_PREFIX), "{hash:?}");
    // `verify_password` reads the salt and the parameters from the PHC
    // string, so `Argon2::default()` verifies a hash that the server made at
    // the parameters of ADR-0022.
    assert!(
        Argon2::default()
            .verify_password(password.as_bytes(), hash.as_str())
            .is_ok(),
        "the returned password does not match the stored hash"
    );
    (password, valid_after)
}

/// On every backend, the open that reset-superadmin runs fails on a missing
/// database. On a set-up database each reset returns a new 16-character
/// alphanumeric one-time password that matches the stored Argon2id hash, sets
/// the must-change flag and moves `tokens_valid_after` forward. A reset
/// creates the superadmin again when none is live.
#[test]
fn reset_superadmin_sets_a_new_one_time_password() {
    // The reset takes no input that could vary, so the test is one scenario
    // and not a property.
    //
    // 1. We open a missing database as reset-superadmin does, which never
    //    creates one, and expect `Error::Open`.
    // 2. We set the database up and reset twice. Each reset must replace the
    //    hash and move the cutoff, so the old password and tokens stop working.
    // 3. We mark the superadmin deleted and reset once more, which must create
    //    it again, as after a first run that stopped before it existed.
    on_each_backend(|backend| {
        // 1. open a missing database
        let database = TestDatabase::fresh(backend);
        let error = block_on(setup::open_checked(&database.url)).err();
        assert!(
            matches!(error, Some(Error::Open(_))),
            "{error:?} before the set-up on {backend}"
        );

        // 2. set up and reset twice
        let (store, _) = set_up_database(&database);
        let (_, _, created_hash, created_valid_after) =
            stored(&database).expect("the set-up created the superadmin");
        let (first, first_valid_after) = reset_and_check(&store, &database);
        // A reset hashes with Argon2id before it reads the clock for its
        // cutoff, so a strict `>` cannot tie, here and below.
        assert!(
            first_valid_after > created_valid_after,
            "the cutoff stayed on {backend}"
        );
        let (_, _, first_hash, _) = stored(&database).expect("a live superadmin");
        assert_ne!(first_hash, created_hash);
        let (second, second_valid_after) = reset_and_check(&store, &database);
        assert_ne!(first, second);
        assert!(
            second_valid_after > first_valid_after,
            "the cutoff stayed on {backend}"
        );

        // 3. delete the superadmin and reset again
        delete(&database);
        reset_and_check(&store, &database);
    });
}
