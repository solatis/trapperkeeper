//! The result of a step for the rows of one chunk, and the three-valued logic
//! of ADR-0005 over it.

use crate::column::{CHUNK_WORDS, Present, present_all};
use crate::plan::Constant;

/// The result of a step for the rows of one chunk. A row is true, false,
/// unknown or faulted. A true row is known, and a faulted row is neither known
/// nor true.
#[derive(Debug, Clone)]
pub(crate) struct Truth {
    // A row whose bit is clear here and in `fault` is unknown, the null of a
    // boolean (ADR-0005).
    known: Present,
    true_rows: Present,
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
                known: all,
                true_rows: all,
                fault: NONE,
            },
            Constant::False => Self {
                known: all,
                true_rows: NONE,
                fault: NONE,
            },
            Constant::Unknown => Self {
                known: NONE,
                true_rows: NONE,
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
            true_rows: zip(truth, &known, |t, k| t & k),
            known,
            fault: NONE,
        }
    }

    /// The result of a step that faults on every row in `rows`.
    pub(crate) fn faulted(rows: Present) -> Self {
        Self {
            known: NONE,
            true_rows: NONE,
            fault: rows,
        }
    }

    /// The rows that match, which are the true rows.
    pub(crate) fn matched(&self) -> &Present {
        &self.true_rows
    }

    /// The rows that faulted.
    pub(crate) fn faults(&self) -> &Present {
        &self.fault
    }

    /// Whether row `row` of the chunk faulted.
    pub(crate) fn faulted_at(&self, row: usize) -> bool {
        self.fault[row / 64] >> (row % 64) & 1 == 1
    }

    /// `not`, which swaps true and false and keeps unknown and the faults.
    pub(crate) fn not(&self) -> Self {
        Self {
            known: self.known,
            true_rows: zip(&self.true_rows, &self.known, |t, k| !t & k),
            fault: self.fault,
        }
    }

    /// `and` in written order: false where an operand is false, and a fault
    /// of the right operand counts only where the left one is not false.
    pub(crate) fn and(&self, right: &Self) -> Self {
        // The function combines 64 rows per word. It decides the faults
        // first, because a faulted row is neither known nor true.
        //
        // 1. A row faults where the left operand faults, or where the right
        //    one faults and the left one is not false. A false left operand
        //    decides the row, so the fault of the right one does not happen
        //    (ADR-0005).
        // 2. Of the other rows, a row is false where either operand is false,
        //    true where both are true, and unknown otherwise.
        let mut out = Self::faulted(NONE);
        for k in 0..CHUNK_WORDS {
            let (left_false, right_false) = (self.falses(k), right.falses(k));
            // 1. decide the faults
            let fault = self.fault[k] | right.fault[k] & !left_false;

            // 2. decide true, false and unknown
            let both = self.true_rows[k] & right.true_rows[k];
            out.known[k] = (left_false | right_false | both) & !fault;
            out.true_rows[k] = both & !fault;
            out.fault[k] = fault;
        }
        out
    }

    /// `or` in written order: true where an operand is true, and a fault of
    /// the right operand counts only where the left one is not true.
    pub(crate) fn or(&self, right: &Self) -> Self {
        // The function combines 64 rows per word. It decides the faults
        // first, because a faulted row is neither known nor true.
        //
        // 1. A row faults where the left operand faults, or where the right
        //    one faults and the left one is not true. A true left operand
        //    decides the row, so the fault of the right one does not happen
        //    (ADR-0005).
        // 2. Of the other rows, a row is true where either operand is true,
        //    false where both are false, and unknown otherwise.
        let mut out = Self::faulted(NONE);
        for k in 0..CHUNK_WORDS {
            // 1. decide the faults
            let fault = self.fault[k] | right.fault[k] & !self.true_rows[k];

            // 2. decide true, false and unknown
            let either = self.true_rows[k] | right.true_rows[k];
            let both_false = self.falses(k) & right.falses(k);
            out.known[k] = (either | both_false) & !fault;
            out.true_rows[k] = either & !fault;
            out.fault[k] = fault;
        }
        out
    }

    /// The rows of word `k` that are false: known and not true.
    fn falses(&self, k: usize) -> u64 {
        self.known[k] & !self.true_rows[k]
    }
}

/// The bitmap that `f` makes of each pair of words of `a` and `b`.
fn zip(a: &Present, b: &Present, f: impl Fn(u64, u64) -> u64) -> Present {
    std::array::from_fn(|k| f(a[k], b[k]))
}
