mod crud;
mod database;
mod models;
mod schema;
mod utils;
mod web;

use std::error::Error;

fn run() -> std::result::Result<(), Box<dyn Error>> {
    let pool = database::pool();

    let mut conn = pool.get()?;

    database::run_migrations(&mut conn)?;
    web::run(pool)?;

    Ok(())
}

fn main() {
    run().unwrap();
}
