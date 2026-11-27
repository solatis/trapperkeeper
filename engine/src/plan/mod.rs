//! The planner. It turns a rule set into one `Program`, a flat list of steps
//! where each step writes its result into the slot with its own number. It
//! lists the fields that the rules read as a tree of paths. ADR-0035 gives the
//! reasons.

mod fields;
mod program;

pub use fields::{FieldNode, FieldTree};
pub(crate) use program::{Comparison, Constant, Literal, Operand, Step};
pub use program::{Program, Unsupported, plan};
