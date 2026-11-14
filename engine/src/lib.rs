//! The rule model: what a rule is, the tree of its expression, their JSON
//! form, the parser of a rule's text, and the validation of a rule's own
//! fields. This crate does not check an expression or evaluate rules.

pub mod expr;
pub mod limits;
pub mod rule;
pub mod text;
pub mod validate;

pub use expr::{Expr, Segment, TypeName};
pub use rule::{Action, Rule};
pub use text::{SyntaxError, parse};
pub use validate::{Error, Reason, validate};
