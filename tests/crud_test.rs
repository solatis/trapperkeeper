use more_asserts as ma;

use trapperkeeper::crud;
use trapperkeeper::database;

#[test]
fn can_create_app() {
    let conn = &mut database::establish_connection();

    let app_id = crud::create_app(conn, "foo");
    ma::assert_gt!(app_id, 0)
}
