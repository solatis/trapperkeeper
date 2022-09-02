use diesel::prelude::*;
use diesel::result::Error;

use crate::models::{App, AuthToken, NewApp};

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

pub fn create_auth_token(
    conn: &mut SqliteConnection,
    app_id: i32,
    title: &String,
) -> Result<String, Error> {
    use crate::schema::auth_tokens;

    let auth_token = AuthToken::new(app_id, title);

    let result = diesel::insert_into(auth_tokens::table)
        .values(&auth_token)
        .execute(conn);

    match result {
        Ok(1) => Ok(auth_token.id),
        Ok(n) => panic!("insertion error: {}", n),
        Err(e) => Err(e),
    }
}

pub fn get_auth_token_by_id(conn: &mut SqliteConnection, id: &String) -> Result<AuthToken, Error> {
    use crate::schema::auth_tokens;

    return auth_tokens::table
        .filter(auth_tokens::id.eq(id))
        .get_result::<AuthToken>(conn);
}

pub fn check_auth_token_by_id(conn: &mut SqliteConnection, id: &String) -> Result<bool, Error> {
    use crate::schema::auth_tokens;

    let r = auth_tokens::table
        .filter(auth_tokens::id.eq(id))
        .get_result::<AuthToken>(conn);

    match r {
        Ok(_) => Ok(true),
        Err(Error::NotFound) => Ok(false),
        Err(e) => Err(e),
    }
}

pub fn check_auth_token_by_app_and_id(
    conn: &mut SqliteConnection,
    app_id: i32,
    auth_token_id: &String,
) -> Result<bool, Error> {
    use crate::schema::auth_tokens;

    let r = auth_tokens::table
        .filter(auth_tokens::app_id.eq(app_id))
        .filter(auth_tokens::id.eq(auth_token_id))
        .get_result::<AuthToken>(conn);

    match r {
        Ok(_) => Ok(true),
        Err(Error::NotFound) => Ok(false),
        Err(e) => Err(e),
    }
}

pub fn delete_auth_token_by_id(conn: &mut SqliteConnection, id: &String) -> Result<bool, Error> {
    use crate::schema::auth_tokens;

    let r = diesel::delete(auth_tokens::table.filter(auth_tokens::id.eq(id))).execute(conn);

    match r {
        Ok(0) => Ok(false),
        Ok(_) => Ok(true),
        Err(e) => Err(e),
    }
}
