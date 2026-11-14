//! A rule as text: the tokenizer, the kit of parser combinators and the
//! error a faulty text gets.

mod error;
mod kit;
mod tokens;

pub use error::{Found, Reason, SyntaxError};
