use derive_more;
use diesel;
use diesel::connection::SimpleConnection;
use diesel::r2d2::CustomizeConnection;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use lazy_static::lazy_static;
use r2d2;

use crate::config;

#[derive(Debug, derive_more::Display, derive_more::Error, derive_more::From)]
pub enum Error {
    #[from]
    DbError(r2d2::Error),

    MigrationError,
}

type InnerConnectionType = diesel::r2d2::ConnectionManager<SqliteConnection>;
type InnerPool = diesel::r2d2::Pool<InnerConnectionType>;

pub type PooledConnection = diesel::r2d2::PooledConnection<InnerConnectionType>;
pub struct Pool(InnerPool);

impl Pool {
    pub fn get(&self) -> Result<PooledConnection, Error> {
        let p: &InnerPool = &self.0;
        match p.get() {
            Ok(conn) => Ok(conn),
            Err(e) => Err(e.into()),
        }
    }
}

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

lazy_static! {
    pub static ref POOL: Pool = PoolBuilder::new().build();
}

#[derive(Copy, Clone, Debug)]
struct ConnectionCustomizer {}

impl<C, E> CustomizeConnection<C, E> for ConnectionCustomizer
where
    C: SimpleConnection,
{
    fn on_acquire(&self, conn: &mut C) -> std::result::Result<(), E> {
        log::debug!("enabling foreign keys on connection");
        conn.batch_execute("PRAGMA foreign_keys = ON")
            .expect("unable to enable foreign keys");

        Ok(())
    }
}

pub struct PoolBuilder {
    cfg: config::Database,
}

impl PoolBuilder {
    pub fn new() -> Self {
        PoolBuilder {
            cfg: config::CONFIG.database.clone(),
        }
    }

    pub fn build(&mut self) -> Pool {
        log::info!(
            "building connection pool from url {} with pool size {}",
            self.cfg.url,
            self.cfg.pool_size
        );

        let customizer = Box::new(ConnectionCustomizer {});

        let pool = InnerPool::builder()
            .max_size(self.cfg.pool_size)
            .connection_customizer(customizer)
            .build(diesel::r2d2::ConnectionManager::new(&self.cfg.url))
            .expect("Failed to create database connection pool");

        let mut conn = pool.get().expect("unable to get connection");
        run_migrations(&mut conn).expect("unable to run migrations");
        Pool(pool)
    }
}

pub fn run_migrations(conn: &mut PooledConnection) -> std::result::Result<(), Error> {
    log::info!("running migrations");
    match conn.run_pending_migrations(MIGRATIONS) {
        Ok(_) => Ok(()),
        _ => Err(Error::MigrationError),
    }
}
