use more_asserts as ma;
use rstest::*;

use trapperkeeper::crud;
use trapperkeeper::database;
use trapperkeeper::models::{AuthToken, Trapp};

#[fixture]
pub fn conn() -> database::PooledConnection {
    database::POOL.get().unwrap()
}

#[fixture]
pub fn trapp(mut conn: database::PooledConnection) -> Trapp {
    return crud::create_trapp(conn.as_mut(), &String::from("foo")).unwrap();
}

#[fixture]
pub fn trapps(mut conn: database::PooledConnection) -> Vec<Trapp> {
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
pub fn auth_token(mut conn: database::PooledConnection, trapp_id: i32) -> AuthToken {
    return crud::create_auth_token(conn.as_mut(), trapp_id, &String::from("foo")).unwrap();
}

////
// Creation
//

#[rstest]
fn can_create_trapp(mut conn: database::PooledConnection) {
    let trapp: Trapp = crud::create_trapp(conn.as_mut(), &String::from("foo")).unwrap();
    ma::assert_gt!(trapp.id, Some(0))
}

#[rstest]
fn can_create_auth_token(mut conn: database::PooledConnection, trapp_id: i32) {
    let auth_token_id = crud::create_auth_token(conn.as_mut(), trapp_id, &String::from("foo"));
    assert_eq!(auth_token_id.is_ok(), true)
}

#[rstest]
fn cannot_create_auth_token_when_trapp_doesnt_exist(mut conn: database::PooledConnection) {
    let auth_token_id = crud::create_auth_token(conn.as_mut(), -2, &String::from("foo"));
    assert_eq!(auth_token_id.is_ok(), false)
}

////
// List
//

#[rstest]
fn can_get_trapps(mut conn: database::PooledConnection, trapps: Vec<Trapp>) {
    let trapps_ = crud::get_trapps(conn.as_mut()).expect("unable to list trapps");

    // Verify all recently created trapps are inside our returned trapps_. Likely there
    // are more.
    assert!(trapps.iter().all(|trapp| trapps_.contains(trapp)));
}

////
// Get
//

#[rstest]
fn can_get_trapp(mut conn: database::PooledConnection, trapp: Trapp) {
    let get = crud::get_trapp_by_id(conn.as_mut(), trapp.id.unwrap());

    assert_eq!(get, Ok(Some(trapp)));
}

#[rstest]
fn get_nonexisting_trapp(mut conn: database::PooledConnection) {
    let get = crud::get_trapp_by_id(conn.as_mut(), -1);

    assert_eq!(get, Ok(None));
}

#[rstest]
fn can_get_auth_token(mut conn: database::PooledConnection, auth_token: AuthToken) {
    let get = crud::get_auth_token_by_id(conn.as_mut(), &auth_token.id).unwrap();

    assert_eq!(get.unwrap(), auth_token);
}

////
// Deletion
//

#[rstest]
fn can_delete_trapp(mut conn: database::PooledConnection, trapp_id: i32) {
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
fn can_delete_auth_token(mut conn: database::PooledConnection, auth_token: AuthToken) {
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
