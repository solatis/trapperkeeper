//! The executor. It binds a `Program` to the column types of a schema and
//! runs the bound program over the columns of a batch, one chunk of
//! `CHUNK_ROWS` rows at a time. ADR-0035 gives the reasons.

mod bind;
mod number;
mod outcome;
mod run;
mod truth;

pub use bind::{BindError, Bound};
pub use outcome::{Fault, Outcome, Outcomes, RuleOutcome};
pub use run::SchemaMismatch;
