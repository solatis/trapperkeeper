//! The rule model: what a rule is, its JSON form, and the validation that
//! every rule passes before it reaches a sensor. This crate does not evaluate
//! rules.

pub mod limits;
pub mod path;
pub mod rule;

pub use path::{FieldPath, Segment};
pub use rule::{Action, Condition, FieldType, Group, Operator, Policy, Rule, Scalar, Value};
