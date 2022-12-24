pub mod auth_token;
pub mod rule;
pub mod trapp;

#[derive(Debug, derive_more::From, derive_more::Display, derive_more::Error)]
pub enum Error {
    #[from]
    SqlxError(sqlx::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

// Re-export everything in the submodules
pub use auth_token::*;
pub use rule::*;
pub use trapp::*;
