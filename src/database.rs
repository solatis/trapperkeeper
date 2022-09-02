use diesel::connection::SimpleConnection; // for batch_execute
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use dotenvy::dotenv;
use std::env;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

pub fn establish_connection() -> Result<SqliteConnection, diesel::result::Error> {
    dotenv().ok();

    let url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    establish_connection_from_url(url)
}

pub fn establish_connection_from_url(
    url: String,
) -> Result<SqliteConnection, diesel::result::Error> {
    let mut conn = SqliteConnection::establish(&url)
        .unwrap_or_else(|_| panic!("Unable to open database {}", url));

    conn.batch_execute("PRAGMA foreign_keys = ON")
        .expect("Unable to enable foreign keys");

    Ok(conn)
}

pub enum Error {
    MigrationError,
}

pub fn run_migrations(conn: &mut SqliteConnection) -> std::result::Result<(), Error> {
    match conn.run_pending_migrations(MIGRATIONS) {
        Ok(_) => Ok(()),
        _ => Err(Error::MigrationError),
    }
}
