use actix_files::Files;
use actix_web::{web, HttpResponse};
use handlebars::Handlebars;
use rust_embed::RustEmbed;
use serde_json::json;

use crate::config;
use crate::models;

/// Authentication for admin
///
/// Verifies login credentials, sets JWT token cookie if successful.
async fn post_login(login: web::Json<models::Login>) -> HttpResponse {
    let credentials = &config::CONFIG.admin;

    if login.username != credentials.username || login.password != credentials.password {
        return HttpResponse::Forbidden().finish();
    }

    let session = models::Session::new(&login.username);

    HttpResponse::Ok().json(session)
}

async fn get_index(hb: web::Data<Handlebars<'_>>) -> HttpResponse {
    log::info!("get_admin_index");

    let data = json!({"name": "Leon Mergen"});
    let body = hb.render("index", &data).unwrap();

    HttpResponse::Ok().body(body)
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

    cfg.app_data(web::Data::new(hb.clone()))
        .service(
            web::scope("/admin")
                .route("/index", web::get().to(get_index))
                .route("/login", web::post().to(post_login)),
        )
        .service(Files::new("/static", "./static"));
}
