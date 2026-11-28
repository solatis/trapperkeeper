//! `StepResult`, what one step of a program computes for the rows of one
//! chunk, and the `not`, `and` and `or` that combine results.

use crate::column::{CHUNK_WORDS, Present, present_all};
use crate::plan::Constant;

/// What one step computed for each row of a chunk, such as the result of
/// `x > 5` on 2048 rows.
///
/// Each row has one of four values:
///
/// - true, as for `5 > 3`.
/// - false, as for `1 > 3`.
/// - unknown, when a value is missing, as for `None > 3` (ADR-0005).
/// - a fault, when the expression cannot be evaluated, as for `"a" > 3`.
///
/// The result is stored as three bitmaps with one bit per row, so that
/// `and`, `or` and `not` combine 64 rows per instruction. A row is true when
/// its bit is set in `known` and in `true_rows`, false when it is set in
/// `known` only, a fault when it is set in `fault`, and unknown when it is
/// set in none of them.
#[derive(Debug, Clone)]
pub(crate) struct StepResult {
    // Set where the row is true or false.
    known: Present,
    // Set where the row is true. Every row set here is also set in `known`.
    true_rows: Present,
    // Set where the row is a fault. No row set here is set in the others.
    fault: Present,
}

/// A bitmap with no bit set.
const NONE: Present = [0; CHUNK_WORDS];

impl StepResult {
    /// A result with every row unknown. `and` and `or` start from it and
    /// fill it in word by word.
    const EMPTY: Self = Self {
        known: NONE,
        true_rows: NONE,
        fault: NONE,
    };

    /// Returns the result that is `c` on each of the first `rows` rows of a
    /// chunk.
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
            // The bitmaps only say that a row is a fault, not which fault.
            // `run` finds the kind in the step for each faulted record.
            Constant::Fault(_) => Self::faulted(all),
        }
    }

    /// Returns the result of a comparison. `true_rows` is what the
    /// comparison's loop wrote, and `known` marks the rows where every field
    /// of the comparison has a value. The other rows become unknown.
    pub(crate) fn compared(true_rows: &Present, known: Present) -> Self {
        Self {
            // The loop compares every row, also the rows without a value,
            // where the memory holds whatever the source left there. Masking
            // with `known` throws those answers away.
            true_rows: zip(true_rows, &known, |t, k| t & k),
            known,
            fault: NONE,
        }
    }

    /// Returns the result of a search: true on `matched`, unknown on the
    /// other rows of `undecided` and on the rows outside `known`, and false on
    /// the rest of `known`.
    pub(crate) fn searched(matched: &Present, undecided: &Present, known: Present) -> Self {
        // A matched row is true even when another element is a null, as for
        // `any` (ADR-0005), so only an undecided row without a match leaves
        // `known`.
        let known = zip(&known, &zip(undecided, matched, |u, m| u & !m), |k, u| {
            k & !u
        });
        Self {
            true_rows: zip(matched, &known, |m, k| m & k),
            known,
            fault: NONE,
        }
    }

    /// Returns the result of `x is None` on the first `rows` rows of a
    /// chunk, where `present` marks the rows on which `x` has a value. It is
    /// true on the rows without a value, false on the others, and never
    /// unknown.
    pub(crate) fn null_test(present: &Present, rows: usize) -> Self {
        let mut all = NONE;
        present_all(rows, &mut all);
        Self {
            // `!p` would also set the bits past the end of the chunk, so `a`
            // keeps only the chunk's rows.
            true_rows: zip(present, &all, |p, a| !p & a),
            known: all,
            fault: NONE,
        }
    }

    /// Returns a result that is a fault on the rows set in `rows` and unknown
    /// on the others.
    pub(crate) fn faulted(rows: Present) -> Self {
        Self {
            known: NONE,
            true_rows: NONE,
            fault: rows,
        }
    }

    /// The rows where the result is true. For a rule's last step, these are
    /// the records that the rule matches.
    pub(crate) fn matched(&self) -> &Present {
        &self.true_rows
    }

    /// The rows where the result is a fault.
    pub(crate) fn faults(&self) -> &Present {
        &self.fault
    }

    /// Whether the result is a fault on row `row` of the chunk.
    pub(crate) fn faulted_at(&self, row: usize) -> bool {
        self.fault[row / 64] >> (row % 64) & 1 == 1
    }

    /// Computes `not`: true becomes false and false becomes true, and
    /// unknown and faults stay as they are.
    pub(crate) fn not(&self) -> Self {
        Self {
            known: self.known,
            true_rows: zip(&self.true_rows, &self.known, |t, k| !t & k),
            fault: self.fault,
        }
    }

    /// Computes `self and right`, where `self` is the left operand as the
    /// rule wrote it. Both operands were computed for every row, but a fault
    /// in `right` only counts on rows where `self` is not false, because a
    /// false left operand already decides the result.
    pub(crate) fn and(&self, right: &Self) -> Self {
        // The loop handles 64 rows per word. It finds the faults first,
        // because a fault row must be clear in `known` and `true_rows`.
        //
        // 1. A row is a fault where the left operand is a fault, or where the
        //    right operand is a fault and the left one is not false. Where the
        //    left operand is false, `a and b` is false whatever `b` is, so the
        //    fault of `b` is ignored (ADR-0005).
        // 2. On the other rows, the result is false if either side is false,
        //    true if both are true, and unknown otherwise (ADR-0005).
        let mut out = Self::EMPTY;
        for k in 0..CHUNK_WORDS {
            // 1. decide the faults
            let (left_false, right_false) = (self.falses(k), right.falses(k));
            let fault = self.fault[k] | right.fault[k] & !left_false;

            // 2. decide true, false and unknown
            let both = self.true_rows[k] & right.true_rows[k];
            // A row is known when it is false through either side or true
            // through both.
            out.known[k] = (left_false | right_false | both) & !fault;
            out.true_rows[k] = both & !fault;
            out.fault[k] = fault;
        }
        out
    }

    /// Computes `self or right`, where `self` is the left operand as the rule
    /// wrote it. Both operands were computed for every row, but a fault in
    /// `right` only counts on rows where `self` is not true, because a true
    /// left operand already decides the result.
    pub(crate) fn or(&self, right: &Self) -> Self {
        // The loop handles 64 rows per word. It finds the faults first,
        // because a fault row must be clear in `known` and `true_rows`.
        //
        // 1. A row is a fault where the left operand is a fault, or where the
        //    right operand is a fault and the left one is not true. Where the
        //    left operand is true, `a or b` is true whatever `b` is, so the
        //    fault of `b` is ignored (ADR-0005).
        // 2. On the other rows, the result is true if either side is true,
        //    false if both are false, and unknown otherwise (ADR-0005).
        let mut out = Self::EMPTY;
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

    /// Returns the false rows of word `k`: known and not true.
    fn falses(&self, k: usize) -> u64 {
        self.known[k] & !self.true_rows[k]
    }
}

/// Builds a bitmap word by word: word `k` is `f(a[k], b[k])`.
fn zip(a: &Present, b: &Present, f: impl Fn(u64, u64) -> u64) -> Present {
    std::array::from_fn(|k| f(a[k], b[k]))
}
