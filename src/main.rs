mod config;
mod crud;
mod crypto;
mod database;
mod models;
mod utils;
mod web;

use crate::config::Config;
use crate::database::Pool;

use actix_web;

use std::error::Error;

async fn run() -> std::result::Result<(), Box<dyn Error>> {
    let _ = env_logger::builder()
        .filter(None, log::LevelFilter::Debug)
        .try_init();

    let cfg = Config::new().expect("Unable to read configuration");
    let mut pool = Pool::builder()
        .from_config(&cfg.database)
        .build()
        .await
        .expect("Unable to construct database connection pool");

    pool.migrate()
        .await
        .expect("Unable to perform database migrations");

    web::run(&mut pool).await.expect("Unable to run webserver");

    Ok(())
}

// We could also use tokio::main here, but let's stick to actix being the "owner"
// of the runtime for now
#[actix_web::main]
async fn main() {
    run().await.expect("System runtime exited abnormally");
}
