//! `Bound::run`: runs a bound program over the columns of one batch and
//! collects each rule's outcome per record.

use std::fmt;

use super::bind::{Bound, BoundStep, FieldType};
use super::outcome::{Fault, Outcomes};
use super::result::StepResult;
use crate::column::{CHUNK_ROWS, CHUNK_WORDS, Column, Present};
use crate::plan::Constant;

/// The error of `run`: a column does not match the schema that the program
/// was bound to. Its type or row count differs from what bind saw, or a
/// field that had a column at bind has none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchemaMismatch {
    /// The number of the field whose column does not match.
    pub field: usize,
}

impl fmt::Display for SchemaMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the column of field {} does not match the schema of the bind",
            self.field
        )
    }
}

impl std::error::Error for SchemaMismatch {}

impl Bound {
    /// Runs the program over a batch of `rows` records and returns, for each
    /// rule, which records it matched and which it could not evaluate.
    /// `columns` holds one column per field, of the types that `bind` saw, or
    /// `None` for a field that the source does not have.
    ///
    /// # Errors
    ///
    /// `SchemaMismatch` when a column has another type or row count than bind
    /// saw, or when `columns` does not have one entry per field.
    pub fn run(
        &self,
        rows: usize,
        columns: &[Option<Column<'_>>],
    ) -> Result<Outcomes, SchemaMismatch> {
        // The batch is split into chunks of up to `CHUNK_ROWS` rows, and each
        // chunk runs the whole program before the next one starts, with one
        // call per step per chunk (ADR-0035).
        //
        // 1. Check that each column has the type that bind saw and `rows`
        //    rows. The loops read a column's memory as that type, so a wrong
        //    type would read garbage.
        // 2. For each chunk, compute once per field which rows have a value.
        //    Every step that reads the field uses this bitmap.
        // 3. Run the steps from first to last. A step reads the bitmaps of its
        //    fields or the results of earlier steps, and its own result goes
        //    into `results` at the index of its slot.
        // 4. Append each rule's matched and faulted records to the outcomes,
        //    with the kind of fault of each faulted record.

        // 1. check the batch
        // Too few or too many columns. The error names the first field that
        // only one of the two lists has.
        if columns.len() != self.types.len() {
            return Err(SchemaMismatch {
                field: columns.len().min(self.types.len()),
            });
        }
        for (field, (c, t)) in columns.iter().zip(&self.types).enumerate() {
            // `FieldType::of` gives `Absent` for a missing column, so a field
            // that had no column at bind must have none here either, and
            // the other way round. A missing column has no rows to count.
            if FieldType::of(c.as_ref()) != *t || c.as_ref().is_some_and(|c| c.rows() != rows) {
                return Err(SchemaMismatch { field });
            }
        }

        // 2. write the presence of each field
        let mut present = vec![[0; CHUNK_WORDS]; columns.len()];
        let mut results = Vec::with_capacity(self.steps.len());
        // One outcome per rule, in the order of the rule set.
        let mut outcomes = Outcomes::new(rows, self.rules.iter().map(|&(id, _)| id));
        for start in (0..rows).step_by(CHUNK_ROWS) {
            let chunk = (rows - start).min(CHUNK_ROWS);
            for (out, c) in present.iter_mut().zip(columns) {
                match c {
                    Some(c) => c.present(start, chunk, out),
                    // A field without a column has no value on any row.
                    None => out.fill(0),
                }
            }

            // 3. run the steps
            results.clear();
            for step in &self.steps {
                let result = run_step(step, &results, &present, columns, start, chunk);
                // Pushing in step order puts each result at the index of its
                // slot.
                results.push(result);
            }

            // 4. append each rule's result
            for (outcome, &(_, slot)) in outcomes.rules_mut().iter_mut().zip(&self.rules) {
                let result = &results[slot];
                outcome.append(result.matched(), result.faults(), start, chunk, |row| {
                    fault_of(&self.steps, &results, slot, row)
                });
            }
        }
        Ok(outcomes)
    }
}

/// Runs one step on the chunk of `rows` rows that starts at row `start`.
/// `results` holds the results of the steps before it, and `present` says for
/// each field which rows of the chunk have a value.
fn run_step(
    step: &BoundStep,
    results: &[StepResult],
    present: &[Present],
    columns: &[Option<Column<'_>>],
    start: usize,
    rows: usize,
) -> StepResult {
    match step {
        BoundStep::Constant(c) => StepResult::constant(*c, rows),
        BoundStep::Loop { fields, chunk } => {
            let mut true_rows = [0; CHUNK_WORDS];
            chunk(columns, start, rows, &mut true_rows);
            StepResult::compared(&true_rows, all_present(present, fields))
        }
        // A fault such as an ordering on text only happens where the values
        // are there (ADR-0005). Rows where a field has no value stay unknown.
        BoundStep::Faulted { fields, .. } => StepResult::faulted(all_present(present, fields)),
        BoundStep::Search { fields, chunk } => {
            let (mut matched, mut undecided) = ([0; CHUNK_WORDS], [0; CHUNK_WORDS]);
            chunk(columns, start, rows, &mut matched, &mut undecided);
            StepResult::searched(&matched, &undecided, all_present(present, fields))
        }
        BoundStep::IsNull(f) => StepResult::null_test(&present[*f], rows),
        BoundStep::Not(a) => results[*a].not(),
        BoundStep::And(a, b) => results[*a].and(&results[*b]),
        BoundStep::Or(a, b) => results[*a].or(&results[*b]),
    }
}

/// Returns the rows on which every field in `fields` has a value.
fn all_present(present: &[Present], fields: &[usize]) -> Present {
    // The fold starts with all bits set, even past the end of the chunk. A
    // comparison always reads at least one field, and that field's bitmap
    // clears those bits.
    fields.iter().fold([u64::MAX; CHUNK_WORDS], |rows, &f| {
        std::array::from_fn(|k| rows[k] & present[f][k])
    })
}

/// Returns which fault the step in `slot` has on row `row` of the chunk. The
/// result of that step must be a fault on that row.
fn fault_of(steps: &[BoundStep], results: &[StepResult], slot: usize, row: usize) -> Fault {
    // The bitmaps only say that a row is a fault. To find which fault, the
    // loop walks down from the rule's last step to the step where the fault
    // started: a `not` leads to its operand, an `and` or `or` leads to the
    // operand whose fault counts, and the walk stops at a fault constant or a
    // comparison that faults, which hold the kind.
    let mut slot = slot;
    loop {
        match &steps[slot] {
            BoundStep::Constant(Constant::Fault(fault)) | BoundStep::Faulted { fault, .. } => {
                return *fault;
            }
            BoundStep::Not(a) => slot = *a,
            // A fault of the left operand always counts. If the left operand
            // is not a fault on this row, the fault came from the right one.
            BoundStep::And(a, b) | BoundStep::Or(a, b) => {
                slot = if results[*a].faulted_at(row) { *a } else { *b };
            }
            // These steps never produce a fault, so the walk cannot reach
            // them.
            BoundStep::Constant(_)
            | BoundStep::Loop { .. }
            | BoundStep::Search { .. }
            | BoundStep::IsNull(_) => {
                unreachable!("a step that never faults has no faulted row")
            }
        }
    }
}
