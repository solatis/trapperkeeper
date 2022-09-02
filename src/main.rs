mod crud;
mod database;
mod models;
mod schema;

fn main() {
    let mut conn = &mut database::establish_connection().unwrap();

    database::run_migrations(&mut conn);

    self::crud::create_app(conn, "foo");
}
