//! The engine's own description of a column (ADR-0036). An adapter builds one
//! per field that the rules read, over the memory of its source, and the loops
//! read that memory in place.

mod layout;
mod list;
mod marking;
mod text;
mod values;

use std::fmt;

pub use layout::{AnyOffsets, Fixed, Offset, Offsets, Storage};
pub use list::{List, Lists, PerRecord, Scalar, Shape};
pub use marking::{
    CHUNK_ROWS, CHUNK_WORDS, Float, Marking, Present, clear_nan, present_all, present_from_bits,
    present_from_mask,
};
pub use text::{INLINE, Piece, Text, Texts, Unit, Units, Utf8, View, Views};
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
    // Besides its marking, every NaN of a float column is no value. A reader of
    // the column runs `clear_nan` after the loop of the marking to clear those
    // rows.
    Floats(Floats<'a>, Marking<'a>),
    Booleans(Booleans<'a>, Marking<'a>),
    /// Texts, where the marking says which rows have no text.
    Text(Text<'a>, Marking<'a>),
    /// Lists, where the marking says which records have no list. The element
    /// column has a marking of its own for the elements that have no value
    /// (ADR-0005).
    List(List<'a>, Marking<'a>),
}

/// The description of one column: one type of value, and for each row either
/// a value or none.
#[derive(Debug, Clone, Copy)]
pub struct Column<'a> {
    kind: Kind<'a>,
    // The row count that the constructor checked, kept so that `rows` needs
    // no match over the kinds and their value types.
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

    /// A column of texts. Fails when the marking covers another number of
    /// rows than `values`.
    ///
    /// # Errors
    ///
    /// `Mismatch` with both numbers of rows.
    pub fn text(values: Text<'a>, marking: Marking<'a>) -> Result<Self, Mismatch> {
        let rows = matched(values.len(), marking.rows())?;
        Ok(Self {
            kind: Kind::Text(values, marking),
            rows,
        })
    }

    /// A column of lists. Fails when the marking covers another number of
    /// rows than `values`.
    ///
    /// # Errors
    ///
    /// `Mismatch` with both numbers of rows.
    pub fn list(values: List<'a>, marking: Marking<'a>) -> Result<Self, Mismatch> {
        let rows = matched(values.len(), marking.rows())?;
        Ok(Self {
            kind: Kind::List(values, marking),
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

/// The row count of a column of `values` values whose marking covers
/// `marking` rows, where `None` is a marking that fits any count. Fails when
/// the two counts differ.
fn matched(values: usize, marking: Option<usize>) -> Result<usize, Mismatch> {
    match marking {
        // The marking `Never` has no length of its own.
        None => Ok(values),
        Some(m) if m == values => Ok(values),
        Some(marking) => Err(Mismatch { values, marking }),
    }
}
