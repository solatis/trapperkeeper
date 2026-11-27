//! The run of a bound program over the columns of a batch.

use std::fmt;

use super::bind::{Bound, BoundStep, FieldType};
use super::outcome::{Fault, Outcomes};
use super::truth::Truth;
use crate::column::{CHUNK_ROWS, CHUNK_WORDS, Column, Present};
use crate::plan::Constant;

/// A column whose row count or type differs from what bind saw, or a field
/// without a column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchemaMismatch {
    /// The number of the field.
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
    /// Runs the program over a batch of `rows` records, with one column per
    /// field as `bind` took them.
    ///
    /// # Errors
    ///
    /// `SchemaMismatch` when a column has another row count or another type
    /// than bind saw, or when the columns are not one per field.
    pub fn run(
        &self,
        rows: usize,
        columns: &[Option<Column<'_>>],
    ) -> Result<Outcomes, SchemaMismatch> {
        // The function checks the batch, then runs the program one chunk at a
        // time and appends the result of each chunk to the outcomes.
        //
        // 1. It checks that each column has `rows` rows and the type that bind
        //    saw, because a loop reads a column's memory as that type.
        // 2. For each chunk, it writes the bitmap of the rows with a value of
        //    each field once. Every step that reads the field shares it.
        // 3. It runs the steps in order. Each writes the result of its slot
        //    from the bitmaps of its fields.
        // 4. It appends the matched and the faulted records of each rule to
        //    the outcomes, with the fault of each faulted record.

        // 1. check the batch
        if columns.len() != self.types.len() {
            return Err(SchemaMismatch {
                field: columns.len().min(self.types.len()),
            });
        }
        for (field, (c, t)) in columns.iter().zip(&self.types).enumerate() {
            if FieldType::of(c.as_ref()) != *t || c.as_ref().is_some_and(|c| c.rows() != rows) {
                return Err(SchemaMismatch { field });
            }
        }

        let mut present = vec![[0; CHUNK_WORDS]; columns.len()];
        let mut truths = Vec::with_capacity(self.steps.len());
        let mut outcomes = Outcomes::new(rows, self.rules.iter().map(|&(id, _)| id));
        // 2. write the presence of each field
        for start in (0..rows).step_by(CHUNK_ROWS) {
            let chunk = (rows - start).min(CHUNK_ROWS);
            for (out, c) in present.iter_mut().zip(columns) {
                match c {
                    Some(c) => c.present(start, chunk, out),
                    // A field that the source lacks has a value on no row.
                    None => out.fill(0),
                }
            }

            // 3. run the steps
            truths.clear();
            for step in &self.steps {
                let truth = run_step(step, &present, columns, start, chunk);
                truths.push(truth);
            }

            // 4. append each rule's result
            for (outcome, &(_, slot)) in outcomes.rules_mut().iter_mut().zip(&self.rules) {
                let truth = &truths[slot];
                outcome.append(truth.matched(), truth.faults(), start, chunk, |row| {
                    fault_of(&self.steps, slot, row)
                });
            }
        }
        Ok(outcomes)
    }
}

/// The result of `step` on the chunk of `rows` rows from row `start`, where
/// `present` holds the bitmap of each field for the chunk.
fn run_step(
    step: &BoundStep,
    present: &[Present],
    columns: &[Option<Column<'_>>],
    start: usize,
    rows: usize,
) -> Truth {
    match step {
        BoundStep::Constant(c) => Truth::constant(*c, rows),
        BoundStep::Loop { fields, chunk } => {
            let mut truth = [0; CHUNK_WORDS];
            chunk(columns, start, rows, &mut truth);
            Truth::compared(&truth, all_present(present, fields))
        }
        BoundStep::Faulted { fields, .. } => Truth::faulted(all_present(present, fields)),
    }
}

/// The rows on which every field of `fields` has a value.
fn all_present(present: &[Present], fields: &[usize]) -> Present {
    fields.iter().fold([u64::MAX; CHUNK_WORDS], |rows, &f| {
        std::array::from_fn(|k| rows[k] & present[f][k])
    })
}

/// The fault of the record at `row` of the chunk in the result of `slot`,
/// which faulted on that row.
fn fault_of(steps: &[BoundStep], slot: usize, _row: usize) -> Fault {
    match &steps[slot] {
        BoundStep::Constant(Constant::Fault(fault)) | BoundStep::Faulted { fault, .. } => *fault,
        // Only a fault constant and a type fault set a fault bit.
        BoundStep::Constant(_) | BoundStep::Loop { .. } => {
            unreachable!("a step that never faults has no faulted row")
        }
    }
}
