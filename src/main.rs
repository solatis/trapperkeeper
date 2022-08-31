mod crud;
mod database;
mod models;
mod schema;

fn main() {
    let conn = &mut database::establish_connection();

    self::crud::create_app(conn, "foo");
}
