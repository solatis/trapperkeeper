//! The executor. `Program::bind` specializes a program for the column types
//! of one schema, and `Bound::run` runs the result over the columns of a
//! batch, `CHUNK_ROWS` rows at a time. ADR-0035 gives the reasons.

mod bind;
mod member;
mod number;
mod outcome;
mod result;
mod run;
mod search;
mod text;

pub use bind::{BindError, Bound};
pub use outcome::{Fault, Outcome, Outcomes, RuleOutcome};
pub use run::SchemaMismatch;
