use actix_web::{cookie, dev::Payload, http, web, HttpRequest, HttpResponse};
use derive_more::{Display, Error};
use futures_util::future::LocalBoxFuture;

use crate::crypto;
use crate::models;

#[derive(Debug, Display, Error)]
pub enum Error {
    #[display(fmt = "HMAC not accessible")]
    HmacNotAccessible,

    #[display(fmt = "No session cookie set")]
    NoCookie,

    #[display(fmt = "Session verification failed")]
    VerificationFailed,
}

impl actix_web::error::ResponseError for Error {
    fn error_response(&self) -> HttpResponse {
        match *self {
            // Not a real error, just no (valid) session set
            Error::VerificationFailed | Error::NoCookie => HttpResponse::Found()
                .append_header(("Location", "/admin/login"))
                .finish(),

            // Internal error: HMAC not accessible through Actix web::Data
            Error::HmacNotAccessible => {
                HttpResponse::build(http::StatusCode::INTERNAL_SERVER_ERROR)
                    .insert_header(http::header::ContentType::html())
                    .body(self.to_string())
            }
        }
    }
}

impl actix_web::FromRequest for models::Session {
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<models::Session, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let req = req.clone();
        Box::pin(async move {
            let hm = req
                .app_data::<web::Data<crypto::HmacType>>()
                .ok_or(Error::HmacNotAccessible)?;
            let cookie = req.cookie("authorization").ok_or(Error::NoCookie)?;
            let jwt = String::from(cookie.value());

            log::debug!("found JWT session cookie");
            let session: models::Session =
                crypto::jwt_decode(&jwt, &hm).map_err(|_| Error::VerificationFailed)?;

            log::debug!("has session with username: {}", session.username);
            Ok(session)
        })
    }
}

pub fn inject_session(
    hm: &crypto::HmacType,
    session: models::Session,
    response: &mut HttpResponse,
) {
    let jwt = crypto::jwt_encode(session, &hm);

    let c = cookie::Cookie::build("authorization", jwt)
        .path("/")
        .secure(false)
        .http_only(true)
        .max_age(cookie::time::Duration::seconds(86400))
        .finish();

    response
        .add_cookie(&c)
        .expect("Unable to add cookie to response");
}
