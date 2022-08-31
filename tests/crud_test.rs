use diesel::prelude::SqliteConnection;
use more_asserts as ma;
use rstest::*;

use trapperkeeper::crud;
use trapperkeeper::database;
use trapperkeeper::models::App;

#[fixture]
pub fn conn() -> SqliteConnection {
    return database::establish_connection();
}

#[fixture]
pub fn app_id(mut conn: SqliteConnection) -> i32 {
    return crud::create_app(&mut conn, "foo");
}

#[fixture]
pub fn app(mut conn: SqliteConnection, app_id: i32) -> App {
    return crud::get_app_by_id(&mut conn, app_id);
}

#[rstest]
fn can_create_app(mut conn: SqliteConnection) {
    let app_id = crud::create_app(&mut conn, "foo");
    ma::assert_gt!(app_id, 0)
}

#[rstest]
fn can_get_app(mut conn: SqliteConnection, app_id: i32) {
    let app = crud::get_app_by_id(&mut conn, app_id);

    assert!(app.id.is_some());
    assert_eq!(app.id.unwrap(), app_id);
}

#[rstest]
fn can_check_app_exists(mut conn: SqliteConnection, app_id: i32) {
    assert_eq!(crud::check_app_by_id(&mut conn, app_id).unwrap(), true);
}

#[rstest]
fn can_check_app_not_exists(mut conn: SqliteConnection) {
    assert_eq!(crud::check_app_by_id(&mut conn, -1).unwrap(), false);
}

#[rstest]
#[should_panic]
fn get_nonexisting_app_should_panic(mut conn: SqliteConnection) {
    crud::get_app_by_id(&mut conn, -1);
}
