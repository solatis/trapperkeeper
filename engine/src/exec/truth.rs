//! The result of a step for the rows of one chunk.

use crate::column::{CHUNK_WORDS, Present, present_all};
use crate::plan::Constant;

/// The result of a step for the rows of one chunk: the rows that are true
/// and the rows that faulted. A faulted row is never true.
#[derive(Debug, Clone)]
pub(crate) struct Truth {
    truth: Present,
    fault: Present,
}

/// A bitmap with no row set.
const NONE: Present = [0; CHUNK_WORDS];

impl Truth {
    /// The result of `c` on the first `rows` rows of a chunk.
    pub(crate) fn constant(c: Constant, rows: usize) -> Self {
        let mut all = NONE;
        present_all(rows, &mut all);
        match c {
            Constant::True => Self {
                truth: all,
                fault: NONE,
            },
            Constant::False | Constant::Unknown => Self {
                truth: NONE,
                fault: NONE,
            },
            Constant::Fault(_) => Self::faulted(all),
        }
    }

    /// The result of a comparison whose loop wrote `truth`, where `known` is
    /// the rows on which every field of the comparison has a value.
    pub(crate) fn compared(truth: &Present, known: Present) -> Self {
        Self {
            // A loop also compares the values of the rows without a value,
            // and the result of such a row is unknown.
            truth: zip(truth, &known, |t, k| t & k),
            fault: NONE,
        }
    }

    /// The result of a step that faults on every row in `rows`.
    pub(crate) fn faulted(rows: Present) -> Self {
        Self {
            truth: NONE,
            fault: rows,
        }
    }

    /// The rows that match, which are the true rows.
    pub(crate) fn matched(&self) -> &Present {
        &self.truth
    }

    /// The rows that faulted.
    pub(crate) fn faults(&self) -> &Present {
        &self.fault
    }
}

/// The bitmap that `f` makes of each pair of words of `a` and `b`.
fn zip(a: &Present, b: &Present, f: impl Fn(u64, u64) -> u64) -> Present {
    std::array::from_fn(|k| f(a[k], b[k]))
}
