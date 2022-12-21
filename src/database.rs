use derive_more;
use lazy_static::lazy_static;

use crate::config;
use sqlx;
use sqlx::migrate::MigrateDatabase;

/// Error specialization
#[derive(Debug, derive_more::Display, derive_more::Error, derive_more::From)]
pub enum Error {
    #[from]
    SqlxError(sqlx::Error),

    #[from]
    MigrationError(sqlx::migrate::MigrateError),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

// Internal type aliases
type Db = sqlx::Sqlite;
type InnerConnection = sqlx::SqliteConnection;
type InnerPool = sqlx::Pool<Db>;
type InnerPoolOptions = sqlx::pool::PoolOptions<Db>;
type InnerPoolConnection = sqlx::pool::PoolConnection<Db>;

// Public exposed connection type alias
pub type Connection = PoolConnection;

// Public exposed query type alias
pub type Query<'q> =
    sqlx::query::Query<'q, Db, <Db as sqlx::database::HasArguments<'q>>::Arguments>;

// Trait to allow coercion to a "regular" connection, mainly so that APIs don't
// need to be aware whether we're using pure sqlx Connection or our own
// wrapper.
pub trait IntoConnection {
    fn into_connection(&mut self) -> &mut InnerConnection;
}

// Wraps a pooled connection, mainly so that we can specialize its traits
pub struct PoolConnection {
    inner: InnerPoolConnection,
}

impl IntoConnection for PoolConnection {
    fn into_connection(&mut self) -> &mut InnerConnection {
        self.inner.as_mut()
    }
}

impl IntoConnection for InnerPoolConnection {
    fn into_connection(&mut self) -> &mut InnerConnection {
        self.as_mut()
    }
}

impl PoolConnection {
    pub fn new(conn: InnerPoolConnection) -> Self {
        PoolConnection { inner: conn }
    }
}

// Wraps a pool, so that we can return a specialized connection type
#[derive(Clone)]
pub struct Pool {
    inner: InnerPool,
}

impl Pool {
    pub fn builder() -> PoolBuilder {
        PoolBuilder::default()
    }

    fn new(pool: InnerPool) -> Self {
        Pool { inner: pool }
    }

    pub async fn acquire(&self) -> Result<PoolConnection> {
        match self.inner.acquire().await {
            Ok(conn) => Ok(PoolConnection::new(conn)),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn migrate(&self) -> Result<()> {
        let mut conn = self
            .inner
            .acquire()
            .await
            .expect("Unable to acquire connection from pool");

        sqlx::migrate!("./migrations")
            .run(&mut conn)
            .await
            .expect("Unable to run migrations");

        Ok(())
    }
}

#[derive(Default)]
pub struct PoolBuilder {
    url: String,
    pool_size: u32,
}

impl PoolBuilder {
    pub fn from_config(mut self, config: &config::Database) -> Self {
        self.url(&config.url).pool_size(config.pool_size)
    }

    pub fn url(mut self, url: &str) -> Self {
        self.url = String::from(url);
        self
    }

    pub fn pool_size(mut self, pool_size: u32) -> Self {
        self.pool_size = pool_size;
        self
    }

    pub async fn build(&mut self) -> Result<Pool> {
        log::info!(
            "building connection pool from url {} with pool size {}",
            self.url,
            self.pool_size
        );

        if Db::database_exists(&self.url)
            .await
            .expect("Unable to check database exists")
            == false
        {
            log::info!("Creating database for url {}", self.url);
            Db::create_database(&self.url)
                .await
                .expect("Unable to create database");
        }

        let pool = InnerPoolOptions::new()
            .min_connections(self.pool_size)
            .max_connections(self.pool_size)
            .connect(&self.url)
            .await
            .expect("Unable to construct database connection pool");

        Ok(Pool { inner: pool })
    }
}
