use diesel::connection::SimpleConnection; // for batch_execute
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use dotenvy::dotenv;
use std::env;

pub fn establish_connection() -> SqliteConnection {
    dotenv().ok();

    let url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let mut conn = SqliteConnection::establish(&url)
        .unwrap_or_else(|_| panic!("Unable to open database {}", url));

    conn.batch_execute("PRAGMA foreign_keys = ON")
        .expect("Unable to enable foreign keys");

    conn
}
