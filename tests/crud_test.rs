use diesel::connection::SimpleConnection; // for batch_execute
use diesel::prelude::SqliteConnection;
use more_asserts as ma;
use rstest::*;

use trapperkeeper::crud;
use trapperkeeper::database;
use trapperkeeper::models::App;

#[fixture]
pub fn conn() -> SqliteConnection {
    let mut conn = database::establish_connection();
    conn.batch_execute("PRAGMA foreign_keys = ON")
        .expect("Unable to enable foreign keys");

    return conn;
}

#[fixture]
pub fn app_id(mut conn: SqliteConnection) -> i32 {
    return crud::create_app(&mut conn, "foo");
}

#[fixture]
pub fn app(mut conn: SqliteConnection, app_id: i32) -> App {
    return crud::get_app_by_id(&mut conn, app_id);
}

////
// Creation
//

#[rstest]
fn can_create_app(mut conn: SqliteConnection) {
    let app_id = crud::create_app(&mut conn, "foo");
    ma::assert_gt!(app_id, 0)
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
fn can_get_app(mut conn: SqliteConnection, app_id: i32) {
    let app = crud::get_app_by_id(&mut conn, app_id);

    assert!(app.id.is_some());
    assert_eq!(app.id.unwrap(), app_id);
}

#[rstest]
#[should_panic]
fn get_nonexisting_app_should_panic(mut conn: SqliteConnection) {
    crud::get_app_by_id(&mut conn, -1);
}

////
// Check exists
//

#[rstest]
fn can_check_app_exists(mut conn: SqliteConnection, app_id: i32) {
    assert_eq!(crud::check_app_by_id(&mut conn, app_id), Ok(true));
}

#[rstest]
fn can_check_app_not_exists(mut conn: SqliteConnection) {
    assert_eq!(crud::check_app_by_id(&mut conn, -1), Ok(false));
}

////
// Deletion
//

#[rstest]
fn can_delete_app(mut conn: SqliteConnection, app_id: i32) {
    assert_eq!(crud::check_app_by_id(&mut conn, app_id), Ok(true));
    assert_eq!(crud::delete_app_by_id(&mut conn, app_id), Ok(true));
    assert_eq!(crud::check_app_by_id(&mut conn, app_id), Ok(false));
    assert_eq!(crud::delete_app_by_id(&mut conn, app_id), Ok(false));
}
