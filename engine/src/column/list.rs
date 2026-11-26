//! The layouts of a list column and the trait `Lists` through which a loop
//! reads any of them. Three layouts point into one element column, which has a
//! description of its own. In the fourth, every record is a column of its own.

use std::ops::Range;

use super::layout::{self, AnyOffsets, Fixed, Offset, Offsets, Storage};
use super::{Column, Kind};

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
    /// The elements of one record: a range of the element column, or the
    /// record's own column.
    // A record of the column-per-record layout is a whole column, which has
    // no place in a shared element column, so the two forms differ.
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

impl sealed::Sealed for Fixed<&Column<'_>> {}

impl<'a> Lists<'a> for Fixed<&'a Column<'a>> {
    type Record = Range<usize>;

    fn len(&self) -> usize {
        Fixed::len(self)
    }

    fn get(&self, row: usize) -> Range<usize> {
        self.range(row)
    }
}

/// The type of the values of a column, in the terms of the rule language.
// A width such as `i8` or `i64` is a layout and not a type, so records of one
// list may hold ints of different widths (ADR-0041).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scalar {
    Int,
    Float,
    Bool,
    Text,
}

/// What the values of a column are and how deep its lists nest: a list of
/// lists of float32 has the scalar `Float` and the depth 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shape {
    pub scalar: Scalar,
    pub depth: u8,
}

impl Shape {
    /// The shape of a list whose elements have this shape.
    fn deeper(self) -> Self {
        Self {
            depth: self.depth + 1,
            ..self
        }
    }
}

impl Column<'_> {
    /// The shape of the column's values.
    #[must_use]
    pub fn shape(&self) -> Shape {
        let scalar = |scalar| Shape { scalar, depth: 0 };
        match self.kind() {
            Kind::Ints(..) => scalar(Scalar::Int),
            Kind::Floats(..) => scalar(Scalar::Float),
            Kind::Booleans(..) => scalar(Scalar::Bool),
            Kind::Text(..) => scalar(Scalar::Text),
            Kind::List(list, _) => list.shape(),
        }
    }
}

/// A list column whose records are each a column of their own, as in a numpy
/// array of dtype `object` that holds arrays. The records share one shape, and
/// each may have its own layout and marking.
#[derive(Debug, Clone, Copy)]
pub struct PerRecord<'a> {
    records: &'a [Column<'a>],
    // The shape of every record. The constructor checks it, and an empty list
    // column has no record to take it from.
    shape: Shape,
}

impl<'a> PerRecord<'a> {
    /// A list column of one record per column, each of shape `shape`.
    ///
    /// # Errors
    ///
    /// The index of the first record whose shape is not `shape`.
    pub fn new(records: &'a [Column<'a>], shape: Shape) -> Result<Self, usize> {
        match records.iter().position(|r| r.shape() != shape) {
            Some(row) => Err(row),
            None => Ok(Self { records, shape }),
        }
    }
}

impl sealed::Sealed for PerRecord<'_> {}

impl<'a> Lists<'a> for PerRecord<'a> {
    type Record = &'a Column<'a>;

    fn len(&self) -> usize {
        self.records.len()
    }

    fn get(&self, row: usize) -> &'a Column<'a> {
        &self.records[row]
    }
}

/// The layout of a list column.
#[derive(Debug, Clone, Copy)]
pub enum List<'a> {
    /// The elements of all records end to end in the element column, with
    /// offsets of any of the four types.
    Offsets(AnyOffsets<'a, &'a Column<'a>>),
    /// The same number of elements per record, as in a two-dimensional numpy
    /// array.
    Fixed(Fixed<&'a Column<'a>>),
    /// A column per record, which may itself be a list.
    PerRecord(PerRecord<'a>),
}

impl List<'_> {
    /// The number of lists.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::Offsets(o) => o.len(),
            Self::Fixed(f) => f.len(),
            Self::PerRecord(p) => p.len(),
        }
    }

    /// The shape of the column of lists: the shape of its elements, one
    /// level deeper.
    #[must_use]
    pub fn shape(&self) -> Shape {
        let elements = match self {
            Self::Offsets(AnyOffsets::I32(o)) => o.storage().shape(),
            Self::Offsets(AnyOffsets::I64(o)) => o.storage().shape(),
            Self::Offsets(AnyOffsets::U32(o)) => o.storage().shape(),
            Self::Offsets(AnyOffsets::U64(o)) => o.storage().shape(),
            Self::Fixed(f) => f.storage().shape(),
            Self::PerRecord(p) => p.shape,
        };
        elements.deeper()
    }

    /// Whether the column holds no list.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
