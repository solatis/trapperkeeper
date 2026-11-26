//! How a column marks the rows that have no value. A source says how it marks
//! "no value" (ADR-0036), and a NaN is no value in every source (ADR-0005).

use super::values::{Bits, Strided};

/// How a column marks the rows that have no value.
#[derive(Debug, Clone, Copy)]
pub enum Marking<'a> {
    /// Every row has a value.
    Never,
    /// One bit per row, and a set bit means the row has a value. Polars and
    /// `DuckDB` mark this way, and so does Arrow data.
    Bitmap(Bits<'a>),
    /// One byte per row, and a byte that is not zero means the row has no
    /// value. numpy's masked arrays and pandas' nullable columns mark this
    /// way.
    // The bytes may lie a stride apart, because the mask of a stepped numpy
    // view is stepped too.
    ByteMask(Strided<'a, u8>),
}

impl Marking<'_> {
    /// The number of rows the marking covers. `None` for `Never`, which covers
    /// any number.
    #[must_use]
    pub fn rows(&self) -> Option<usize> {
        match self {
            Self::Never => None,
            Self::Bitmap(b) => Some(b.len()),
            Self::ByteMask(m) => Some(m.len()),
        }
    }
}
