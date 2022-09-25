use actix_files::Files;
use actix_web::{http::Uri, web, Error, HttpResponse};
use handlebars::Handlebars;
use rust_embed::RustEmbed;
use serde::Deserialize;
use serde_json::json;

use crate::config;
use crate::crud;
use crate::crypto;
use crate::models;

use super::session;
use super::util::get_conn;

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
    session: models::Session,
    hb: web::Data<Handlebars<'_>>,
) -> Result<HttpResponse, Error> {
    log::info!("get_admin_trapps");

    let mut conn = get_conn()?;
    let trapps = crud::get_trapps(&mut conn).map_err(actix_web::error::ErrorInternalServerError)?;

    log::debug!("got {} trapps", trapps.len());

    let data = json!({"page_title": "Trapps",
                      "trapps": trapps});
    let body = hb
        .render("trapps", &data)
        .map_err(actix_web::error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().body(body))
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
                .route("/login", web::get().to(get_login))
                .route("/login", web::post().to(post_login)),
        )
        .service(Files::new("/static", "./static"));
}
