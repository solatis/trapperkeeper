use actix_files::Files;
use actix_web::{
    http::{header::ContentType, StatusCode},
    web, HttpResponse,
};
use derive_more;
use handlebars::{Handlebars, RenderError};
use rust_embed::RustEmbed;
use serde::Deserialize;
use serde_json::json;

use crate::config;
use crate::crud;
use crate::crypto;
use crate::database;
use crate::models;

use super::session;

#[derive(Debug, derive_more::Display, derive_more::Error)]
pub enum Error {
    TemplateError(RenderError),
    CrudError(crud::Error),
    DatabaseError(database::Error),
}

impl From<crud::Error> for Error {
    fn from(e: crud::Error) -> Self {
        Error::CrudError(e)
    }
}

impl From<RenderError> for Error {
    fn from(e: RenderError) -> Self {
        Error::TemplateError(e)
    }
}

impl From<database::Error> for Error {
    fn from(e: database::Error) -> Self {
        Error::DatabaseError(e)
    }
}

impl actix_web::error::ResponseError for Error {
    fn error_response(&self) -> HttpResponse {
        match *self {
            // Database error
            Error::CrudError(_) | Error::TemplateError(_) | Error::DatabaseError(_) => {
                HttpResponse::build(StatusCode::INTERNAL_SERVER_ERROR)
                    .insert_header(ContentType::html())
                    .body(self.to_string())
            }
        }
    }
}

/// Authentication for admin
///
/// Verifies login credentials, sets JWT token cookie if successful, and redirects
/// to main overview upon successful login.
async fn post_login(
    hm: web::Data<crypto::HmacType>,
    login: web::Form<models::Login>,
) -> HttpResponse {
    log::info!("post_admin_login");

    let credentials = &config::CONFIG.admin;

    if login.username != credentials.username || login.password != credentials.password {
        return HttpResponse::Found()
            .append_header(("Location", "/admin/login?auth_failed=true"))
            .finish();
    }

    let mut result = HttpResponse::Found()
        .append_header(("Location", "/admin/overview"))
        .finish();

    session::inject_session(&hm, models::Session::new(&login.username), &mut result);
    result
}

#[derive(Deserialize)]
struct GetLoginQueryParams {
    auth_failed: Option<bool>,
}

/// Get overview
///
/// Renders login page, and optionally presents an authentication failure message.
async fn get_login(
    hb: web::Data<Handlebars<'_>>,
    params: web::Query<GetLoginQueryParams>,
) -> HttpResponse {
    log::info!("get_admin_login");
    let auth_failed = params.auth_failed.unwrap_or(false);

    log::info!("auth_failed: {}", auth_failed);

    let data = json!({ "auth_failed": auth_failed });
    let body = hb.render("login", &data).unwrap();

    HttpResponse::Ok().body(body)
}

/// Get overview
///
/// Dashboard / welcome screen / overview
async fn get_overview(session: models::Session, hb: web::Data<Handlebars<'_>>) -> HttpResponse {
    log::info!("get_admin_overview");

    let data = json!({"page_title": "Overview"});
    let body = hb.render("overview", &data).unwrap();

    HttpResponse::Ok().body(body)
}

/// Get trapps
///
/// Presents overview of existing / installed trapps.
async fn get_trapps(
    _session: models::Session,
    hb: web::Data<Handlebars<'_>>,
) -> Result<HttpResponse, Error> {
    log::info!("get_admin_trapps");

    let mut conn = database::POOL.get()?;
    let trapps = crud::get_trapps(&mut conn)?;

    let data = json!({"page_title": "Trapps",
                      "trapps": trapps});
    let body = hb.render("trapps", &data)?;

    Ok(HttpResponse::Ok().body(body))
}

/// Get trapp create
///
/// Presents form to create new trapp.
async fn get_trapp_create(
    _session: models::Session,
    hb: web::Data<Handlebars<'_>>,
) -> Result<HttpResponse, Error> {
    log::info!("get_trapp_create");

    let data = json!({"page_title": "Create trapp"});

    let body = hb.render("trapp_create", &data)?;

    Ok(HttpResponse::Ok().body(body))
}

/// Post trapp create
///
/// Creates new trapp, redirects trapps overview when successful.
async fn post_trapp_create(
    _session: models::Session,
    hb: web::Data<Handlebars<'_>>,
    new_trapp: web::Form<models::NewTrapp>,
) -> Result<HttpResponse, Error> {
    log::info!("post_trapp_create");

    let mut conn = database::POOL.get()?;

    // Create the app
    let trapp: models::Trapp = crud::create_trapp(&mut conn, &new_trapp.name)?;

    // Create a default auth token
    let trapp_id: i32 = trapp.id.unwrap();
    let auth_token_title = String::from("Default token");

    let auth_token: models::AuthToken =
        crud::create_auth_token(&mut conn, trapp_id, &auth_token_title)?;

    let data = json!({"trapp": trapp,
                      "auth_token": auth_token});

    let body = hb.render("trapp_created", &data)?;

    Ok(HttpResponse::Found()
        .append_header(("Location", "/admin/trapps"))
        .finish())
}

/// Templates
///
/// Embed all templates inside our template directory so that we do not
/// need to point to any template directory in production deployments.
///
#[derive(RustEmbed)]
#[folder = "./templates"]
struct Templates;

/// Initialize handlebars templates
///
/// In debug mode, reads templates live from directory so that templates
/// are refreshed without recompilation.
///
/// In production mode, uses embedded templates struct initialized above.
///
fn init_templates<'reg>() -> Handlebars<'reg> {
    let mut hb = Handlebars::new();

    match config::CONFIG.debug {
        true => {
            log::info!("debug mode enabled, using directory templates");
            hb.register_templates_directory(".html", "./templates")
                .unwrap();
            hb.set_dev_mode(true)
        }
        false => {
            log::debug!("production mode enabled, using embedded templates");
            hb.register_embed_templates::<Templates>().unwrap();
            hb.set_dev_mode(false)
        }
    }

    hb
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    let hb = init_templates();
    let hm = crypto::random_hmac();

    cfg.app_data(web::Data::new(hb.clone()))
        .app_data(web::Data::new(hm.clone()))
        .service(
            web::scope("/admin")
                .route("/overview", web::get().to(get_overview))
                .route("/trapps", web::get().to(get_trapps))
                .route("/trapp_create", web::get().to(get_trapp_create))
                .route("/trapp_create", web::post().to(post_trapp_create))
                .route("/login", web::get().to(get_login))
                .route("/login", web::post().to(post_login)),
        )
        .service(Files::new("/static", "./static"));
}
