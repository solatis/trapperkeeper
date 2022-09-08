use actix_web::web::{Data, ServiceConfig};
use actix_web::{App, HttpServer};

use crate::config;
use crate::database;

pub mod admin;
pub mod api;

pub fn add_database(cfg: &mut ServiceConfig, pool: database::Pool) {
    cfg.app_data(Data::new(pool.clone()));
}

#[actix_web::main]
pub async fn run() -> std::io::Result<()> {
    let cfg = config::CONFIG.api.clone();
    let pool: database::Pool = database::PoolBuilder::new().build();

    database::run_migrations(&mut pool.get().unwrap()).expect("Unable to run migrations");

    log::info!("launching api at {}:{}", cfg.addr, cfg.port);

    HttpServer::new(move || {
        App::new()
            .configure(|svc| add_database(svc, pool.clone()))
            .configure(api::configure)
            .configure(admin::configure)
    })
    .bind((cfg.addr, cfg.port))?
    .run()
    .await
}
