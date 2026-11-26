//! The layouts of a list column and the trait `Lists` through which a loop
//! reads any of them. Each layout points into one element column, which has a
//! description of its own.

use std::ops::Range;

use super::Column;
use super::layout::{self, AnyOffsets, Offset, Offsets, Storage};

mod sealed {
    /// The part of `Lists` that only the engine can name, so no other crate
    /// implements it.
    pub trait Sealed {}
}

impl layout::sealed::Sealed for &Column<'_> {}

impl Storage for &Column<'_> {
    fn count(&self) -> usize {
        self.rows()
    }

    // Any rows of a column are valid elements, and a layout checks its ranges
    // against `count` itself.
    fn holds(&self, _: Range<usize>) -> bool {
        true
    }
}

/// A layout of a list column. It gives the elements of a record, read in
/// place from the element column.
// The trait is sealed, so only the engine adds a layout. A loop written
// against the trait runs on every layout.
pub trait Lists<'a>: Copy + sealed::Sealed {
    /// The elements of one record.
    type Record;
    /// The number of records.
    fn len(&self) -> usize;
    /// Whether the layout holds no record.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// The elements of the record at `row`. Panics when `row` is not below
    /// `len`.
    fn get(&self, row: usize) -> Self::Record;
}

impl<O: Offset> sealed::Sealed for Offsets<'_, O, &Column<'_>> {}

impl<'a, O: Offset> Lists<'a> for Offsets<'a, O, &'a Column<'a>> {
    type Record = Range<usize>;

    fn len(&self) -> usize {
        Offsets::len(self)
    }

    fn get(&self, row: usize) -> Range<usize> {
        self.range(row)
    }
}

/// The layout of a list column.
#[derive(Debug, Clone, Copy)]
pub enum List<'a> {
    /// The elements of all records end to end in the element column, with
    /// offsets of any of the four types.
    Offsets(AnyOffsets<'a, &'a Column<'a>>),
}

impl List<'_> {
    /// The number of lists.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::Offsets(o) => o.len(),
        }
    }

    /// Whether the column holds no list.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
