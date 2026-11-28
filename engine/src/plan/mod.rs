//! The planner. It turns the expression trees of a rule set into one
//! `Program`, a flat list of steps that the executor runs in order, and it
//! lists the fields that the rules read, so the caller knows which columns
//! to pass. It knows nothing about columns or types. ADR-0035 gives the
//! reasons.

mod fields;
mod program;

pub use fields::{FieldNode, FieldTree};
pub(crate) use program::{Comparison, Constant, Literal, Operand, Step, TextLiteral};
pub use program::{Program, Unsupported, plan};
