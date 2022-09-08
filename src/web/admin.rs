use actix_files::Files;
use actix_web::{web, HttpResponse};
use handlebars::Handlebars;
use rust_embed::RustEmbed;
use serde_json::json;

use crate::config;

async fn get_admin_index(hb: web::Data<Handlebars<'_>>) -> HttpResponse {
    log::info!("get_admin_index");

    let data = json!({"name": "Leon Mergen"});
    let body = hb.render("index", &data).unwrap();

    HttpResponse::Ok().body(body)
}

#[derive(RustEmbed)]
#[folder = "./templates"]
struct Templates;

pub fn init_templates<'reg>() -> Handlebars<'reg> {
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
        .service(web::scope("/admin").route("/index", web::get().to(get_admin_index)))
        .service(Files::new("/static", "./static"));
}
