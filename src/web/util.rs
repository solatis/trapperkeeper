use actix_web::Error;

use crate::database;

pub fn get_conn() -> Result<database::PooledConnection, Error> {
    database::POOL
        .get()
        .map_err(actix_web::error::ErrorInternalServerError)
}
