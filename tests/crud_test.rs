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
pub fn app(mut conn: SqliteConnection) -> App {
    let app_id = crud::create_app(&mut conn, "foo");
    return crud::get_app_by_id(&mut conn, app_id);
}

#[rstest]
fn can_create_app(mut conn: SqliteConnection) {
    let app_id = crud::create_app(&mut conn, "foo");
    ma::assert_gt!(app_id, 0)
}

#[rstest]
fn can_get_app(mut conn: SqliteConnection) {
    let app_id = crud::create_app(&mut conn, "foo");
    let app = crud::get_app_by_id(&mut conn, app_id);

    assert!(app.id.is_some());
    assert_eq!(app.id.unwrap(), app_id);
}

#[rstest]
#[should_panic]
fn get_nonexisting_app_should_panic(mut conn: SqliteConnection) {
    crud::get_app_by_id(&mut conn, -1);
}
