use actix_web::web::{Data, ServiceConfig};
use actix_web::{App, HttpServer};

use crate::config;
use crate::database::Pool;

pub mod admin;
pub mod session;
pub mod util;

pub fn configure(cfg: &mut ServiceConfig) {
    cfg.configure(admin::configure);
}

pub async fn run(pool: &mut Pool) -> std::io::Result<()> {
    // Initialize database pool
    let cfg = config::CONFIG.api.clone();
    log::warn!("launching api at {}:{}", cfg.addr, cfg.port);

    let data = Data::new(pool.clone());

    HttpServer::new(move || App::new().app_data(Data::clone(&data)).configure(configure))
        .bind((cfg.addr, cfg.port))?
        .run()
        .await
}
