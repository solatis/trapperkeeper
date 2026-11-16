//! A rule as text. `tokens` splits a text into tokens, `kit` holds the
//! parser combinators, `grammar` is the parser written with them, and
//! `error` is what a faulty text gets. `print` is the `Display` of `Expr`,
//! which writes a tree back as text, so it needs no export.

mod error;
mod grammar;
mod kit;
mod print;
mod tokens;

pub use error::{Found, Reason, SyntaxError};
pub use grammar::parse;
