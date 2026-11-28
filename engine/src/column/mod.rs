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
pub use list::{AnySized, List, Lists, PerRecord, Scalar, Shape, SizedLists};
pub use marking::{
    CHUNK_ROWS, CHUNK_WORDS, Float, Marking, Present, clear_nan, present_all, present_from_bits,
    present_from_mask,
};
pub(crate) use marking::{Pair, Test, pack};
pub use text::{INLINE, Piece, Text, Texts, Unit, Units, Utf8, View, Views};
pub use values::{Bits, Booleans, Floats, Ints, Number, Strided};
pub(crate) use values::{NumberType, with_float, with_int, with_number};

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
    // Besides its marking, every NaN of a float column is no value, and
    // `Column::present` clears those rows.
    Floats(Floats<'a>, Marking<'a>),
    Booleans(Booleans<'a>, Marking<'a>),
    /// Texts, where the marking says which rows have no text.
    Text(Text<'a>, Marking<'a>),
    /// Lists, where the marking says which rows have no list. The column
    /// that holds the elements, or the column of each record, has a marking of
    /// its own for the elements that have no value (ADR-0005).
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

    /// Writes into `out` one bit per row in `start .. start + rows`, set
    /// where the row has a value. Bit `i` of `out` is row `start + i`, and the
    /// bits from `rows` on are 0. A float row holding NaN counts as having no
    /// value.
    ///
    /// # Panics
    ///
    /// Panics when `rows` is above `CHUNK_ROWS` or the chunk reaches past the
    /// column.
    pub fn present(&self, start: usize, rows: usize, out: &mut Present) {
        // The marking loops check the chunk against their own length, but
        // `present_all` has no length to check, so this assertion covers
        // that case.
        assert!(
            start.checked_add(rows).is_some_and(|end| end <= self.rows),
            "a chunk of {rows} rows from row {start} of {}",
            self.rows
        );
        let (Kind::Ints(_, marking)
        | Kind::Floats(_, marking)
        | Kind::Booleans(_, marking)
        | Kind::Text(_, marking)
        | Kind::List(_, marking)) = &self.kind;
        match marking {
            Marking::Never => present_all(rows, out),
            Marking::Bitmap(bits) => present_from_bits(*bits, start, rows, out),
            Marking::ByteMask(mask) => present_from_mask(*mask, start, rows, out),
        }
        // A NaN counts as no value in every source (ADR-0005). The marking
        // loop above writes the whole bitmap, so the NaN pass must run after
        // it and only clear bits.
        if let Kind::Floats(values, _) = &self.kind {
            match values {
                Floats::F16(v) => clear_nan(*v, start, rows, out),
                Floats::F32(v) => clear_nan(*v, start, rows, out),
                Floats::F64(v) => clear_nan(*v, start, rows, out),
            }
        }
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
