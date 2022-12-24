use async_std;
use more_asserts as ma;
use rstest::*;

use trapperkeeper::config;
use trapperkeeper::crud;
use trapperkeeper::database;
use trapperkeeper::models::{
    AuthToken, NewRuleFilterField, RuleFilterField, RuleFilterTrapp, Trapp,
};

async fn get_pool() -> database::Pool {
    let cfg = config::Config::new().expect("Unable to read configuration");
    let pool = database::Pool::builder()
        .from_config(&cfg.database)
        .build()
        .await
        .expect("Unable to construct database connection pool");

    pool
}

#[fixture]
async fn pool() -> database::Pool {
    get_pool().await
}

async fn get_conn(pool: &mut database::Pool) -> database::PoolConnection {
    pool.acquire()
        .await
        .expect("Unable to get reference to database pool")
}

pub async fn get_trapp_id(conn: &mut impl database::IntoConnection) -> i64 {
    crud::create_trapp(conn, &"foo").await.unwrap()
}

pub async fn get_trapp(conn: &mut impl database::IntoConnection) -> Trapp {
    let trapp_id = get_trapp_id(conn).await;
    crud::get_trapp_by_id(conn, &trapp_id)
        .await
        .expect("Unable to get trapp by id")
        .expect("Trapp not found")
}

pub async fn get_trapps(conn: &mut impl database::IntoConnection) -> Vec<Trapp> {
    vec![get_trapp(conn).await, get_trapp(conn).await]
}

pub async fn get_auth_token_id(conn: &mut impl database::IntoConnection) -> String {
    let trapp_id: i64 = get_trapp_id(conn).await;
    return crud::create_auth_token(conn, &trapp_id, &"foo")
        .await
        .expect("Unable to create auth token");
}

// Creation
//

#[rstest]
async fn can_create_trapp(#[future] pool: database::Pool) {
    let mut pool = pool.await;
    let mut conn = get_conn(&mut pool).await;

    let trapp_id = crud::create_trapp(&mut conn, &"foo").await.unwrap();

    ma::assert_gt!(trapp_id, 0)
}

#[rstest]
async fn can_create_auth_token(#[future] pool: database::Pool) {
    let mut pool = pool.await;
    let mut conn = get_conn(&mut pool).await;

    let trapp_id: i64 = get_trapp_id(&mut conn).await;

    let auth_token_id = crud::create_auth_token(&mut conn, &trapp_id, &"foo").await;
    assert_eq!(auth_token_id.is_ok(), true)
}

#[rstest]
async fn cannot_create_auth_token_when_trapp_doesnt_exist(#[future] pool: database::Pool) {
    let mut pool = pool.await;
    let mut conn = get_conn(&mut pool).await;

    let auth_token_id = crud::create_auth_token(&mut conn, &-2, &"foo").await;
    assert_eq!(auth_token_id.is_ok(), false)
}

#[rstest]
async fn can_create_rule(#[future] pool: database::Pool) {
    let mut pool = pool.await;
    let mut conn = get_conn(&mut pool).await;

    let rule = NewRuleFilterField::new(&"foo", &"key", &"value");
    let rule_id = crud::create_rule(&mut conn, rule).await;
    assert_eq!(rule_id.is_ok(), true)
}

// List
//

#[rstest]
async fn can_get_trapps(#[future] pool: database::Pool) {
    let mut pool = pool.await;
    let mut conn = get_conn(&mut pool).await;

    // Realize (insertion) future before querying
    let trapps = get_trapps(&mut conn).await;
    let trapps_ = crud::get_trapps(&mut conn)
        .await
        .expect("unable to list trapps");

    // Verify all recently created trapps are inside our returned trapps_. Likely there
    // are more.
    assert!(trapps.iter().all(|trapp| trapps_.contains(trapp)));
}

#[rstest]
async fn can_get_auth_tokens(#[future] pool: database::Pool) {
    let mut pool = pool.await;
    let mut conn = get_conn(&mut pool).await;

    // Realize (insertion) future before querying
    let trapps = get_trapps(&mut conn).await;

    // Create two auth tokens for each of these trapps
    for trapp in trapps.iter() {
        let trapp_id: i64 = trapp.id;
        crud::create_auth_token(&mut conn, &trapp_id, &"first")
            .await
            .expect("Unable to create auth token");
        crud::create_auth_token(&mut conn, &trapp_id, &"second")
            .await
            .expect("Unable to create auth token");
    }

    // Now ensure that each each trapp indeed gets back two auth tokens
    for trapp in trapps.iter() {
        let trapp_id: i64 = trapp.id;
        let xs = crud::get_auth_tokens_by_trapp(&mut conn, &trapp_id)
            .await
            .expect("Unable to list auth tokens");
        assert_eq!(xs.len(), 2);
    }
}

// Get
//

#[rstest]
async fn can_get_trapp(#[future] pool: database::Pool) {
    let mut pool = pool.await;
    let mut conn = get_conn(&mut pool).await;

    // Realize (insertion) future before querying
    let trapp = get_trapp(&mut conn).await;

    let get = crud::get_trapp_by_id(&mut conn, &trapp.id)
        .await
        .expect("Unable to get trapp by id");

    assert_eq!(get, Some(trapp));
}

#[rstest]
async fn get_nonexisting_trapp(#[future] pool: database::Pool) {
    let mut pool = pool.await;
    let mut conn = get_conn(&mut pool).await;

    let get = crud::get_trapp_by_id(&mut conn, &-1)
        .await
        .expect("Unable to get trapp by id");

    assert_eq!(get, None);
}

#[rstest]
async fn can_get_auth_token(#[future] pool: database::Pool) {
    let mut pool = pool.await;
    let mut conn = get_conn(&mut pool).await;

    // Realize (insertion) future before querying
    let auth_token_id = get_auth_token_id(&mut conn).await;
    let get = crud::get_auth_token_by_id(&mut conn, &auth_token_id)
        .await
        .expect("Unable to get auth token by id")
        .expect("Auth token not found");

    assert_eq!(get.id, auth_token_id);
}

// Deletion
//

#[rstest]
async fn can_delete_trapp(#[future] pool: database::Pool) {
    let mut pool = pool.await;
    let mut conn = get_conn(&mut pool).await;

    let trapp_id = get_trapp_id(&mut conn).await;

    assert!(crud::get_trapp_by_id(&mut conn, &trapp_id)
        .await
        .expect("unable to get trapp by id")
        .is_some());
    assert_eq!(
        crud::delete_trapp_by_id(&mut conn, &trapp_id)
            .await
            .expect("unable to delete trapp"),
        true
    );
    assert!(crud::get_trapp_by_id(&mut conn, &trapp_id)
        .await
        .expect("unable to get trapp by id")
        .is_none());
    assert_eq!(
        crud::delete_trapp_by_id(&mut conn, &trapp_id)
            .await
            .expect("unable to delete trapp"),
        false
    );
}

#[rstest]
async fn can_delete_auth_token(#[future] pool: database::Pool) {
    let mut pool = pool.await;
    let mut conn = get_conn(&mut pool).await;

    let auth_token_id = get_auth_token_id(&mut conn).await;

    assert!(crud::get_auth_token_by_id(&mut conn, &auth_token_id)
        .await
        .expect("unable to get auth token by id")
        .is_some());
    assert_eq!(
        crud::delete_auth_token_by_id(&mut conn, &auth_token_id)
            .await
            .expect("unable to delete auth token"),
        true
    );
    assert!(crud::get_auth_token_by_id(&mut conn, &auth_token_id)
        .await
        .expect("unable to get auth token by id")
        .is_none());
    assert_eq!(
        crud::delete_auth_token_by_id(&mut conn, &auth_token_id)
            .await
            .expect("unable to delete auth token"),
        false
    );
}
