//! reset-superadmin through the library on each backend: what it refuses
//! before the database is set up, what it creates on a set-up database
//! without a superadmin, and what a second run changes.

mod common;

use argon2::Argon2;
use argon2::password_hash::PasswordVerifier;
use common::{
    ADMIN, SYSTEM_TENANT_ID, TestDatabase, block_on, on_each_backend, on_pool, set_up_database,
};
use sqlx::AssertSqlSafe;
use trapperkeeper_server::setup::{self, Error};
use trapperkeeper_server::{Store, reset_superadmin};

/// The start of every stored hash: Argon2id at the parameters of
/// ADR-0022.
const HASH_PREFIX: &str = "$argon2id$v=19$m=65536,t=3,p=4$";

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

/// Runs the reset on `store`, checks that the returned one-time password is
/// the one the stored superadmin holds, and returns it with
/// `tokens_valid_after`.
fn reset_and_check(store: &Store, database: &TestDatabase) -> (String, i64) {
    let password = block_on(reset_superadmin(store)).expect("the reset succeeds");
    assert!(
        password.len() == 16 && password.chars().all(|c| c.is_ascii_alphanumeric()),
        "{password:?} is not a 16-character alphanumeric password"
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
        "the returned password does not match the stored hash"
    );
    (password, valid_after)
}

/// On every backend, reset-superadmin's open of a missing database fails.
/// After the set-up, which creates the superadmin, a reset returns a
/// 16-character alphanumeric password that matches the stored Argon2id hash,
/// sets the must-change flag and moves `tokens_valid_after` forward. A second
/// reset returns another password. With the superadmin deleted through sqlx,
/// a reset creates it again.
#[test]
fn reset_superadmin_sets_a_new_one_time_password() {
    // A scenario, because no input varies (ADR-0018).
    //
    // 1. open a missing database as reset-superadmin does
    // 2. set up the database, which creates the superadmin, then reset twice
    // 3. delete the superadmin and reset once more
    on_each_backend(|backend| {
        // 1. reset-superadmin never creates a database, so the open fails.
        let database = TestDatabase::fresh(backend);
        let error = block_on(setup::open_checked(&database.url)).err();
        assert!(
            matches!(error, Some(Error::Open(_))),
            "{error:?} before the set-up on {backend}"
        );

        // 2. Each reset must replace the hash and move the cutoff, so the
        //    password of the set-up and every token issued before stop
        //    working.
        let (store, _) = set_up_database(&database);
        let (_, _, created_hash, created_valid_after) =
            stored(&database).expect("the set-up created the superadmin");
        let (first, first_valid_after) = reset_and_check(&store, &database);
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

        // 3. The missing-superadmin branch recovers a first run that stopped
        //    before the superadmin existed.
        delete(&database);
        reset_and_check(&store, &database);
    });
}
