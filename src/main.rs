mod config;
mod crud;
mod crypto;
mod database;
mod models;
mod schema;
mod utils;
mod web;

use std::error::Error;

fn run() -> std::result::Result<(), Box<dyn Error>> {
    let _ = env_logger::builder()
        .filter(None, log::LevelFilter::Debug)
        .try_init();
    let pool = database::PoolBuilder::new().build();

    let mut conn = pool.get()?;

    database::run_migrations(conn.as_mut())?;
    web::run()?;

    Ok(())
}

fn main() {
    run().unwrap();
}
