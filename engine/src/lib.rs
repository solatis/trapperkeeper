//! The rule model: what a rule is, the tree of its expression, their JSON
//! form, and the validation of a rule's own fields. This crate does not
//! parse a rule's text, check an expression or evaluate rules.

pub mod expr;
pub mod limits;
pub mod rule;
pub mod text;
pub mod validate;

pub use expr::{Expr, Segment, TypeName};
pub use rule::{Action, Rule};
pub use validate::{Error, Reason, validate};
