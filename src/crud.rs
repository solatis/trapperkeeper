use derive_more;
use diesel;
use diesel::prelude::*;

use crate::models::{AuthToken, NewTrapp, Trapp};

#[derive(Debug, derive_more::Display, derive_more::Error, PartialEq)]
pub enum Error {
    DbError(diesel::result::Error),
}

impl From<diesel::result::Error> for Error {
    fn from(e: diesel::result::Error) -> Self {
        Error::DbError(e)
    }
}

pub fn create_trapp(conn: &mut SqliteConnection, title: &String) -> Result<Trapp, Error> {
    use crate::schema::trapps;
    let new_trapp = &NewTrapp::new(title);

    let inserted_trapp = diesel::insert_into(trapps::table)
        .values(new_trapp)
        .get_result::<Trapp>(conn)?;

    Ok(inserted_trapp)
}

pub fn get_trapps(conn: &mut SqliteConnection) -> Result<Vec<Trapp>, Error> {
    use crate::schema::trapps;

    let result = trapps::dsl::trapps.load::<Trapp>(conn)?;
    Ok(result)
}

pub fn get_trapp_by_id(conn: &mut SqliteConnection, id: i32) -> Result<Option<Trapp>, Error> {
    use crate::schema::trapps;

    let result = trapps::table
        .filter(trapps::id.eq(id))
        .get_result::<Trapp>(conn);

    match result {
        Ok(trapp) => Ok(Some(trapp)),
        Err(diesel::result::Error::NotFound) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn delete_trapp_by_id(conn: &mut SqliteConnection, id: i32) -> Result<bool, Error> {
    use crate::schema::trapps;

    let r = diesel::delete(trapps::table.filter(trapps::id.eq(id))).execute(conn);

    match r {
        Ok(0) => Ok(false),
        Ok(_) => Ok(true),
        Err(e) => Err(e.into()),
    }
}

pub fn create_auth_token(
    conn: &mut SqliteConnection,
    trapp_id: i32,
    title: &String,
) -> Result<AuthToken, Error> {
    use crate::schema::auth_tokens;

    let auth_token = AuthToken::new(trapp_id, title);

    diesel::insert_into(auth_tokens::table)
        .values(&auth_token)
        .execute(conn)?;

    Ok(auth_token)
}

pub fn get_auth_token_by_id(
    conn: &mut SqliteConnection,
    id: &String,
) -> Result<Option<AuthToken>, Error> {
    use crate::schema::auth_tokens;

    let result = auth_tokens::table
        .filter(auth_tokens::id.eq(id))
        .get_result::<AuthToken>(conn);

    match result {
        Ok(auth_token) => Ok(Some(auth_token)),
        Err(diesel::result::Error::NotFound) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn get_auth_token_by_trapp_and_id(
    conn: &mut SqliteConnection,
    trapp_id: i32,
    id: &String,
) -> Result<Option<AuthToken>, Error> {
    use crate::schema::auth_tokens;

    let result = auth_tokens::table
        .filter(auth_tokens::trapp_id.eq(trapp_id))
        .filter(auth_tokens::id.eq(id))
        .get_result::<AuthToken>(conn);

    match result {
        Ok(auth_token) => Ok(Some(auth_token)),
        Err(diesel::result::Error::NotFound) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn delete_auth_token_by_id(conn: &mut SqliteConnection, id: &String) -> Result<bool, Error> {
    use crate::schema::auth_tokens;

    let r = diesel::delete(auth_tokens::table.filter(auth_tokens::id.eq(id))).execute(conn);

    match r {
        Ok(0) => Ok(false),
        Ok(_) => Ok(true),
        Err(e) => Err(e.into()),
    }
}

pub fn delete_auth_token_by_trapp_and_id(
    conn: &mut SqliteConnection,
    trapp_id: i32,
    id: &String,
) -> Result<bool, Error> {
    use crate::schema::auth_tokens;

    let r = diesel::delete(
        auth_tokens::table
            .filter(auth_tokens::id.eq(id))
            .filter(auth_tokens::trapp_id.eq(trapp_id)),
    )
    .execute(conn);

    match r {
        Ok(0) => Ok(false),
        Ok(_) => Ok(true),
        Err(e) => Err(e.into()),
    }
}
