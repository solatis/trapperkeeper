//! A rule as text: the parser, its error and the printer.

mod error;
mod grammar;
mod kit;
mod print;
mod tokens;

pub use error::{Found, Reason, SyntaxError};
pub use grammar::parse;
