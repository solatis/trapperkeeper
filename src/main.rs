mod config;
mod crud;
mod database;
mod models;
mod schema;
mod utils;
mod web;

use std::error::Error;

fn run() -> std::result::Result<(), Box<dyn Error>> {
    let pool = database::PoolBuilder::new().build();

    let mut conn = pool.get()?;

    database::run_migrations(&mut conn)?;
    web::run()?;

    Ok(())
}

fn main() {
    run().unwrap();
}
