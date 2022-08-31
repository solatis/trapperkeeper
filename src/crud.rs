use diesel::prelude::*;

use crate::models::{App, NewApp};

pub fn create_app(conn: &mut SqliteConnection, title: &str) -> i32 {
    use crate::schema::apps;
    let new_app = &NewApp::new(title);

    let inserted_app = diesel::insert_into(apps::table)
        .values(new_app)
        .get_result::<App>(conn)
        .unwrap();

    inserted_app.id.unwrap()
}
