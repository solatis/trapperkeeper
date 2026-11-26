//! The engine's own description of a column (ADR-0036). An adapter builds one
//! per field that the rules read, over the memory of its source, and the loops
//! read that memory in place.

mod values;

pub use values::{Bits, Booleans, Floats, Ints, Strided};
