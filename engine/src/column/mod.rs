//! The engine's own description of a column (ADR-0036). An adapter builds one
//! per field that the rules read, over the memory of its source, and the loops
//! read that memory in place.

mod marking;
mod values;

use std::fmt;

pub use marking::Marking;
pub use values::{Bits, Booleans, Floats, Ints, Strided};

/// A marking that covers another number of rows than the values of its
/// column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mismatch {
    pub values: usize,
    pub marking: usize,
}

impl fmt::Display for Mismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the marking covers {} rows and the column holds {} values",
            self.marking, self.values
        )
    }
}

impl std::error::Error for Mismatch {}

/// What a column holds: its values and how it marks "no value".
#[derive(Debug, Clone, Copy)]
pub enum Kind<'a> {
    Ints(Ints<'a>, Marking<'a>),
    Floats(Floats<'a>, Marking<'a>),
    Booleans(Booleans<'a>, Marking<'a>),
}

/// The description of one column: one type of value, and for each row either
/// a value or none.
#[derive(Debug, Clone, Copy)]
pub struct Column<'a> {
    kind: Kind<'a>,
    rows: usize,
}

impl<'a> Column<'a> {
    /// A column of ints. Fails when the marking covers another number of rows
    /// than `values`.
    ///
    /// # Errors
    ///
    /// `Mismatch` with both numbers of rows.
    pub fn ints(values: Ints<'a>, marking: Marking<'a>) -> Result<Self, Mismatch> {
        let rows = matched(values.len(), marking.rows())?;
        Ok(Self {
            kind: Kind::Ints(values, marking),
            rows,
        })
    }

    /// A column of floats. Fails when the marking covers another number of
    /// rows than `values`.
    ///
    /// # Errors
    ///
    /// `Mismatch` with both numbers of rows.
    pub fn floats(values: Floats<'a>, marking: Marking<'a>) -> Result<Self, Mismatch> {
        let rows = matched(values.len(), marking.rows())?;
        Ok(Self {
            kind: Kind::Floats(values, marking),
            rows,
        })
    }

    /// A column of booleans. Fails when the marking covers another number of
    /// rows than `values`.
    ///
    /// # Errors
    ///
    /// `Mismatch` with both numbers of rows.
    pub fn booleans(values: Booleans<'a>, marking: Marking<'a>) -> Result<Self, Mismatch> {
        let rows = matched(values.len(), marking.rows())?;
        Ok(Self {
            kind: Kind::Booleans(values, marking),
            rows,
        })
    }

    /// The number of rows.
    #[must_use]
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// What the column holds.
    #[must_use]
    pub fn kind(&self) -> &Kind<'a> {
        &self.kind
    }
}

/// `values` when the marking covers that many rows or any number of rows.
fn matched(values: usize, marking: Option<usize>) -> Result<usize, Mismatch> {
    match marking {
        // The marking `Never` has no length of its own, so it fits every
        // column.
        None => Ok(values),
        Some(m) if m == values => Ok(values),
        Some(marking) => Err(Mismatch { values, marking }),
    }
}
