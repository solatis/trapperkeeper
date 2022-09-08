use actix_web::web::{Data, ServiceConfig};
use actix_web::{App, HttpServer};

use crate::config;
use crate::database;

pub mod admin;
pub mod api;

pub fn init_pool() -> database::Pool {
    let pool: database::Pool = database::PoolBuilder::new().build();
    database::run_migrations(&mut pool.get().unwrap()).expect("Unable to run migrations");

    pool
}

pub fn configure(cfg: &mut ServiceConfig, pool: database::Pool) {
    cfg.app_data(Data::new(pool.clone()))
        .configure(api::configure)
        .configure(admin::configure);
}

#[actix_web::main]
pub async fn run() -> std::io::Result<()> {
    // Initialize database pool
    let cfg = config::CONFIG.api.clone();
    log::info!("launching api at {}:{}", cfg.addr, cfg.port);

    let pool = init_pool();

    HttpServer::new(move || App::new().configure(|svc| configure(svc, pool.clone())))
        .bind((cfg.addr, cfg.port))?
        .run()
        .await
}
