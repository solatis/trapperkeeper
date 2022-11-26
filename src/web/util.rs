use actix_web::web;
use core::future::Future;
use std::pin::Pin;

use crate::database;

/// Easy access for actix routes to a database connection
///
/// This allows routes to use a pooled connection directly as an argument to
/// their routes.
impl actix_web::FromRequest for database::PoolConnection {
    type Error = actix_web::Error;
    type Future = Pin<Box<dyn Future<Output = Result<database::PoolConnection, Self::Error>>>>;

    fn from_request(
        req: &actix_web::HttpRequest,
        _payload: &mut actix_web::dev::Payload,
    ) -> Self::Future {
        let req = req.clone();

        Box::pin(async move {
            let pool: &web::Data<database::Pool> = req
                .app_data::<web::Data<database::Pool>>()
                .expect("Unable to retrieve database pool from app data");

            match pool.acquire().await {
                Ok(conn) => Ok(conn),
                Err(_) => Err(actix_web::error::ErrorInternalServerError(String::from(
                    "Unable to acquire database connection",
                ))),
            }
        })
    }
}
