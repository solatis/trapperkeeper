use actix_web::web::ServiceConfig;
use actix_web::{App, HttpServer};

use crate::config;

pub mod admin;
pub mod api;

pub fn configure(cfg: &mut ServiceConfig) {
    cfg.configure(api::configure).configure(admin::configure);
}

#[actix_web::main]
pub async fn run() -> std::io::Result<()> {
    // Initialize database pool
    let cfg = config::CONFIG.api.clone();
    log::info!("launching api at {}:{}", cfg.addr, cfg.port);

    HttpServer::new(move || App::new().configure(configure))
        .bind((cfg.addr, cfg.port))?
        .run()
        .await
}
