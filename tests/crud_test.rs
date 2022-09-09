use diesel::prelude::SqliteConnection;
use more_asserts as ma;
use rstest::*;

use trapperkeeper::crud;
use trapperkeeper::database;
use trapperkeeper::models::{App, AuthToken};

#[fixture]
pub fn conn() -> SqliteConnection {
    let mut conn = database::ConnectionBuilder::new().build();
    database::run_migrations(&mut conn).unwrap();
    conn
}

#[fixture]
pub fn app(mut conn: SqliteConnection) -> App {
    return crud::create_app(&mut conn, &String::from("foo")).unwrap();
}

#[fixture]
pub fn app_id(app: App) -> i32 {
    app.id.unwrap()
}

#[fixture]
pub fn auth_token(mut conn: SqliteConnection, app_id: i32) -> AuthToken {
    return crud::create_auth_token(&mut conn, app_id, &String::from("foo")).unwrap();
}

////
// Creation
//

#[rstest]
fn can_create_app(mut conn: SqliteConnection) {
    let app: App = crud::create_app(&mut conn, &String::from("foo")).unwrap();
    ma::assert_gt!(app.id, Some(0))
}

#[rstest]
fn can_create_auth_token(mut conn: SqliteConnection, app_id: i32) {
    let auth_token_id = crud::create_auth_token(&mut conn, app_id, &String::from("foo"));
    assert_eq!(auth_token_id.is_ok(), true)
}

#[rstest]
fn cannot_create_auth_token_when_app_doesnt_exist(mut conn: SqliteConnection) {
    let auth_token_id = crud::create_auth_token(&mut conn, -2, &String::from("foo"));
    assert_eq!(auth_token_id.is_ok(), false)
}

////
// Get
//

#[rstest]
fn can_get_app(mut conn: SqliteConnection, app: App) {
    let get = crud::get_app_by_id(&mut conn, app.id.unwrap());

    assert_eq!(get, Ok(Some(app)));
}

#[rstest]
fn get_nonexisting_app(mut conn: SqliteConnection) {
    let get = crud::get_app_by_id(&mut conn, -1);

    assert_eq!(get, Ok(None));
}

#[rstest]
fn can_get_auth_token(mut conn: SqliteConnection, auth_token: AuthToken) {
    let get = crud::get_auth_token_by_id(&mut conn, &auth_token.id).unwrap();

    assert_eq!(get.unwrap(), auth_token);
}

////
// Deletion
//

#[rstest]
fn can_delete_app(mut conn: SqliteConnection, app_id: i32) {
    assert!(crud::get_app_by_id(&mut conn, app_id)
        .expect("unable to get app by id")
        .is_some());
    assert_eq!(crud::delete_app_by_id(&mut conn, app_id), Ok(true));
    assert!(crud::get_app_by_id(&mut conn, app_id)
        .expect("unable to get app by id")
        .is_none());
    assert_eq!(crud::delete_app_by_id(&mut conn, app_id), Ok(false));
}

#[rstest]
fn can_delete_auth_token(mut conn: SqliteConnection, auth_token: AuthToken) {
    assert!(crud::get_auth_token_by_id(&mut conn, &auth_token.id)
        .expect("unable to get auth token by id")
        .is_some());
    assert_eq!(
        crud::delete_auth_token_by_id(&mut conn, &auth_token.id),
        Ok(true)
    );
    assert!(crud::get_auth_token_by_id(&mut conn, &auth_token.id)
        .expect("unable to get auth token by id")
        .is_none());
    assert_eq!(
        crud::delete_auth_token_by_id(&mut conn, &auth_token.id),
        Ok(false)
    );
}
