use diesel::prelude::SqliteConnection;
use more_asserts as ma;
use rstest::*;

use trapperkeeper::crud;
use trapperkeeper::database;
use trapperkeeper::models::App;

#[fixture]
pub fn conn() -> SqliteConnection {
    let url: String = format!("sqlite://./{}.sqlite", "tests");
    let mut conn = database::establish_connection_from_url(String::from(url)).unwrap();
    database::run_migrations(&mut conn);
    conn
}

#[fixture]
pub fn app_id(mut conn: SqliteConnection) -> i32 {
    return crud::create_app(&mut conn, "foo");
}

#[fixture]
pub fn app(mut conn: SqliteConnection, app_id: i32) -> App {
    return crud::get_app_by_id(&mut conn, app_id);
}

#[fixture]
pub fn auth_token_id(mut conn: SqliteConnection, app_id: i32) -> String {
    return crud::create_auth_token(&mut conn, app_id, &String::from("foo")).unwrap();
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

#[rstest]
fn can_get_auth_token(mut conn: SqliteConnection, auth_token_id: String) {
    let auth_token = crud::get_auth_token_by_id(&mut conn, &auth_token_id);

    assert!(auth_token.is_ok());
    assert_eq!(auth_token.unwrap().id, auth_token_id);
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

#[rstest]
fn can_check_auth_token_exists(mut conn: SqliteConnection, auth_token_id: String) {
    assert_eq!(
        crud::check_auth_token_by_id(&mut conn, &auth_token_id),
        Ok(true)
    );
}

#[rstest]
fn can_check_auth_token_not_exists(mut conn: SqliteConnection) {
    assert_eq!(
        crud::check_auth_token_by_id(&mut conn, &String::from("adsfiugdsfigdsf")),
        Ok(false)
    );
}

#[rstest]
fn can_check_app_auth_token_exists(mut conn: SqliteConnection, auth_token_id: String) {
    let auth_token = crud::get_auth_token_by_id(&mut conn, &auth_token_id).unwrap();
    let app_id: i32 = auth_token.app_id;
    assert_eq!(
        crud::check_auth_token_by_app_and_id(&mut conn, app_id, &auth_token_id),
        Ok(true)
    );
}

#[rstest]
fn can_check_app_auth_token_not_exists(
    mut conn: SqliteConnection,
    app_id: i32,
    auth_token_id: String,
) {
    // Test the tests: app_id and auth_token_id follow to separate "chains" of fixtures, and
    // as such should never share the same app id.
    let auth_token = crud::get_auth_token_by_id(&mut conn, &auth_token_id).unwrap();
    assert_ne!(auth_token.app_id, app_id);

    assert_eq!(
        crud::check_auth_token_by_app_and_id(&mut conn, app_id, &auth_token_id),
        Ok(false)
    );
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

#[rstest]
fn can_delete_auth_token(mut conn: SqliteConnection, auth_token_id: String) {
    assert_eq!(
        crud::check_auth_token_by_id(&mut conn, &auth_token_id),
        Ok(true)
    );
    assert_eq!(
        crud::delete_auth_token_by_id(&mut conn, &auth_token_id),
        Ok(true)
    );
    assert_eq!(
        crud::check_auth_token_by_id(&mut conn, &auth_token_id),
        Ok(false)
    );
    assert_eq!(
        crud::delete_auth_token_by_id(&mut conn, &auth_token_id),
        Ok(false)
    );
}
