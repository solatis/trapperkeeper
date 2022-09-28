use actix_utils::future;
use actix_web;

use crate::database;

/// Easy access for actix routes to a database connection
///
/// This allows routes to use a pooled connection directly as an argument to
/// their routes.
impl actix_web::FromRequest for database::PooledConnection {
    type Error = actix_web::Error;
    type Future = future::Ready<Result<database::PooledConnection, Self::Error>>;

    fn from_request(
        _req: &actix_web::HttpRequest,
        _payload: &mut actix_web::dev::Payload,
    ) -> Self::Future {
        match database::POOL.get() {
            Ok(conn) => future::ok(conn),
            Err(_) => future::err(actix_web::error::ErrorInternalServerError(String::from(
                "Unable to acquire database connection",
            ))),
        }
    }
}
