//! A rule as text: the parser and its error.

mod error;
mod grammar;
mod kit;
mod tokens;

pub use error::{Found, Reason, SyntaxError};
pub use grammar::parse;
