use diesel::prelude::*;
use diesel::result::Error;

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

pub fn get_app_by_id(conn: &mut SqliteConnection, id: i32) -> App {
    use crate::schema::apps;

    return apps::table
        .filter(apps::id.eq(id))
        .get_result::<App>(conn)
        .unwrap();
}

pub fn check_app_by_id(conn: &mut SqliteConnection, id: i32) -> Result<bool, Error> {
    use crate::schema::apps;

    let r = apps::table.filter(apps::id.eq(id)).get_result::<App>(conn);

    match r {
        Ok(_) => Ok(true),
        Err(Error::NotFound) => Ok(false),
        Err(e) => Err(e),
    }
}

pub fn delete_app_by_id(conn: &mut SqliteConnection, id: i32) -> Result<bool, Error> {
    use crate::schema::apps;

    let r = diesel::delete(apps::table.filter(apps::id.eq(id))).execute(conn);

    match r {
        Ok(0) => Ok(false),
        Ok(_) => Ok(true),
        Err(e) => Err(e),
    }
}
