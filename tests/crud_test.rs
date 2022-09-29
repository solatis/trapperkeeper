use more_asserts as ma;
use rstest::*;

use trapperkeeper::crud;
use trapperkeeper::database;
use trapperkeeper::models::{AuthToken, Trapp};

fn get_conn() -> database::PooledConnection {
    database::POOL
        .get()
        .expect("Unable to get reference to database pool")
}

#[fixture]
pub fn trapp() -> Trapp {
    let mut conn = get_conn();
    return crud::create_trapp(conn.as_mut(), &String::from("foo")).unwrap();
}

#[fixture]
pub fn trapps() -> Vec<Trapp> {
    let mut conn = get_conn();
    vec![
        crud::create_trapp(conn.as_mut(), &String::from("trapp1")).unwrap(),
        crud::create_trapp(conn.as_mut(), &String::from("trapp2")).unwrap(),
    ]
}

#[fixture]
pub fn trapp_id(trapp: Trapp) -> i32 {
    trapp.id.unwrap()
}

#[fixture]
pub fn auth_token(trapp_id: i32) -> AuthToken {
    let mut conn = get_conn();
    return crud::create_auth_token(conn.as_mut(), trapp_id, &String::from("foo")).unwrap();
}

// Creation
//

#[rstest]
fn can_create_trapp() {
    let mut conn = get_conn();
    let trapp: Trapp = crud::create_trapp(conn.as_mut(), &String::from("foo")).unwrap();
    ma::assert_gt!(trapp.id, Some(0))
}

#[rstest]
fn can_create_auth_token(trapp_id: i32) {
    let mut conn = get_conn();
    let auth_token_id = crud::create_auth_token(conn.as_mut(), trapp_id, &String::from("foo"));
    assert_eq!(auth_token_id.is_ok(), true)
}

#[rstest]
fn cannot_create_auth_token_when_trapp_doesnt_exist() {
    let mut conn = get_conn();
    let auth_token_id = crud::create_auth_token(conn.as_mut(), -2, &String::from("foo"));
    assert_eq!(auth_token_id.is_ok(), false)
}

// List
//

#[rstest]
fn can_get_trapps(trapps: Vec<Trapp>) {
    let mut conn = get_conn();
    let trapps_ = crud::get_trapps(conn.as_mut()).expect("unable to list trapps");

    // Verify all recently created trapps are inside our returned trapps_. Likely there
    // are more.
    assert!(trapps.iter().all(|trapp| trapps_.contains(trapp)));
}

// Get
//

#[rstest]
fn can_get_trapp(trapp: Trapp) {
    let mut conn = get_conn();
    let get = crud::get_trapp_by_id(conn.as_mut(), trapp.id.unwrap());

    assert_eq!(get, Ok(Some(trapp)));
}

#[rstest]
fn get_nonexisting_trapp() {
    let mut conn = get_conn();
    let get = crud::get_trapp_by_id(conn.as_mut(), -1);

    assert_eq!(get, Ok(None));
}

#[rstest]
fn can_get_auth_token(auth_token: AuthToken) {
    let mut conn = get_conn();
    let get = crud::get_auth_token_by_id(conn.as_mut(), &auth_token.id).unwrap();

    assert_eq!(get.unwrap(), auth_token);
}

// Deletion
//

#[rstest]
fn can_delete_trapp(trapp_id: i32) {
    let mut conn = get_conn();
    assert!(crud::get_trapp_by_id(conn.as_mut(), trapp_id)
        .expect("unable to get trapp by id")
        .is_some());
    assert_eq!(crud::delete_trapp_by_id(conn.as_mut(), trapp_id), Ok(true));
    assert!(crud::get_trapp_by_id(conn.as_mut(), trapp_id)
        .expect("unable to get trapp by id")
        .is_none());
    assert_eq!(crud::delete_trapp_by_id(conn.as_mut(), trapp_id), Ok(false));
}

#[rstest]
fn can_delete_auth_token(auth_token: AuthToken) {
    let mut conn = get_conn();
    assert!(crud::get_auth_token_by_id(conn.as_mut(), &auth_token.id)
        .expect("unable to get auth token by id")
        .is_some());
    assert_eq!(
        crud::delete_auth_token_by_id(conn.as_mut(), &auth_token.id),
        Ok(true)
    );
    assert!(crud::get_auth_token_by_id(conn.as_mut(), &auth_token.id)
        .expect("unable to get auth token by id")
        .is_none());
    assert_eq!(
        crud::delete_auth_token_by_id(conn.as_mut(), &auth_token.id),
        Ok(false)
    );
}
