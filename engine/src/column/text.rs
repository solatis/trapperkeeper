//! The layouts of a text column and the trait `Texts` through which a loop
//! reads any of them. Nothing in this file copies a text.

use std::ops::Range;

use super::layout::{self, AnyOffsets, Offset, Offsets, Storage};

mod sealed {
    /// The part of `Unit` and `Texts` that only the engine can name, so no
    /// other crate implements them.
    pub trait Sealed {}
}

/// A unit of text: a byte of UTF-8, or a unit of UTF-32 for numpy's `U`.
// The trait is sealed, because the engine owns the closed list of cases
// (ADR-0036).
pub trait Unit: Copy + PartialEq + sealed::Sealed {
    /// The unit that pads a fixed-width cell.
    const ZERO: Self;
}

impl sealed::Sealed for u8 {}
impl Unit for u8 {
    const ZERO: Self = 0;
}

impl sealed::Sealed for u32 {}
impl Unit for u32 {
    const ZERO: Self = 0;
}

/// A layout of a text column. It gives the units of the text of a row where
/// the source holds them.
// A layout whose `Unit` is `u8` holds valid UTF-8 in every row, so a loop may
// read a row as a `&str` without a check. A loop compares a column's units
// with a literal that the planner encoded in the same encoding, so no loop
// converts a text (ADR-0041). The trait is sealed, so only the engine adds a
// layout, and a loop written against it runs on every layout.
pub trait Texts<'a>: Copy + sealed::Sealed {
    type Unit: Unit;
    /// The number of texts.
    fn len(&self) -> usize;
    /// Whether the layout holds no text.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// The units of the text at `row`. Panics when `row` is not below `len`.
    fn get(&self, row: usize) -> &'a [Self::Unit];
}

/// The bytes of the texts of an offsets layout. Every range that the layout
/// names holds UTF-8.
#[derive(Debug, Clone, Copy)]
pub struct Utf8<'a>(&'a [u8]);

impl<'a> Utf8<'a> {
    /// The bytes of texts.
    ///
    /// # Safety
    ///
    /// Every range that a layout over these bytes names holds UTF-8.
    /// `Offsets::new` checks it in a debug build.
    #[must_use]
    pub unsafe fn new(bytes: &'a [u8]) -> Self {
        Self(bytes)
    }
}

impl layout::sealed::Sealed for Utf8<'_> {}

impl Storage for Utf8<'_> {
    fn count(&self) -> usize {
        self.0.len()
    }

    fn holds(&self, range: Range<usize>) -> bool {
        self.0.get(range).is_some_and(|b| str::from_utf8(b).is_ok())
    }
}

impl<O: Offset> sealed::Sealed for Offsets<'_, O, Utf8<'_>> {}

impl<'a, O: Offset> Texts<'a> for Offsets<'a, O, Utf8<'a>> {
    type Unit = u8;

    fn len(&self) -> usize {
        Offsets::len(self)
    }

    fn get(&self, row: usize) -> &'a [u8] {
        let range = self.range(row);
        // SAFETY: the contract of `Offsets::new` keeps every range of a row
        // inside the bytes.
        unsafe { self.storage().0.get_unchecked(range) }
    }
}

/// The layout of a text column.
#[derive(Debug, Clone, Copy)]
pub enum Text<'a> {
    /// Texts end to end in one buffer, with offsets of any of the four types,
    /// as Polars and `DuckDB` hold them.
    Offsets(AnyOffsets<'a, Utf8<'a>>),
}

impl Text<'_> {
    /// The number of texts.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::Offsets(o) => o.len(),
        }
    }

    /// Whether the column holds no text.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
