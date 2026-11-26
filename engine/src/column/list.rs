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

// The offsets and the fixed-width layouts of a list point into its element
// column through this impl.
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
// against the trait runs on every layout. `'a` is the lifetime of the borrowed
// columns, which a `Record` of the column-per-record layout carries.
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

/// Lists that each name their elements by an offset into the element column
/// and a size. The lists may lie in any order and may share elements, as
/// Arrow's list view allows.
#[derive(Debug, Clone, Copy)]
pub struct SizedLists<'a, O> {
    offsets: &'a [O],
    sizes: &'a [O],
    elements: &'a Column<'a>,
}

impl<'a, O: Offset> SizedLists<'a, O> {
    /// A list column with one list per offset and size. Code that reads the
    /// elements of a record may slice the element column at the record's
    /// range without a bound check, because of this contract.
    ///
    /// # Safety
    ///
    /// `offsets` and `sizes` have the same length. Every offset and every size
    /// fits a `usize`, and each offset plus its size is at most the number of
    /// rows of `elements`.
    #[must_use]
    pub unsafe fn new(offsets: &'a [O], sizes: &'a [O], elements: &'a Column<'a>) -> Self {
        // `valid` checks the whole contract, so that a caller that breaks it
        // fails in a debug build and under Miri. It asks for no order and no
        // disjoint lists, because a list view allows both.
        debug_assert!(
            Self::valid(offsets, sizes, elements),
            "the lists break the contract of `new`"
        );
        Self {
            offsets,
            sizes,
            elements,
        }
    }

    /// Whether the offsets and the sizes keep the contract of `new`.
    fn valid(offsets: &[O], sizes: &[O], elements: &Column<'_>) -> bool {
        offsets.len() == sizes.len()
            && offsets.iter().zip(sizes).all(|(o, s)| {
                // `fits` runs before `index`, because a negative number would
                // cast to a huge index.
                o.fits()
                    && s.fits()
                    && o.index()
                        .checked_add(s.index())
                        .is_some_and(|end| end <= elements.rows())
            })
    }
}

impl<O: Offset> sealed::Sealed for SizedLists<'_, O> {}

impl<'a, O: Offset> Lists<'a> for SizedLists<'a, O> {
    type Record = Range<usize>;

    fn len(&self) -> usize {
        self.offsets.len()
    }

    fn get(&self, row: usize) -> Range<usize> {
        let start = self.offsets[row].index();
        start..start + self.sizes[row].index()
    }
}

/// A list layout of offsets and sizes with any of the four offset types.
#[derive(Debug, Clone, Copy)]
pub enum AnySized<'a> {
    I32(SizedLists<'a, i32>),
    I64(SizedLists<'a, i64>),
    U32(SizedLists<'a, u32>),
    U64(SizedLists<'a, u64>),
}

/// The type of the values of a column, in the terms of the rule language.
// The rule language has one int type and one float type (ADR-0041), so the
// records of one column-per-record list may hold ints of different widths.
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
    pub depth: usize,
}

impl Shape {
    /// The shape of a list whose elements have this shape.
    fn deeper(self) -> Self {
        Self {
            // Every level of a list is a column in memory, so the depth stays
            // far below the largest `usize`.
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
    // Every record has this shape, which `new` checks. The field keeps it,
    // because an empty list column has no record to take it from.
    shape: Shape,
}

impl<'a> PerRecord<'a> {
    /// A list column with one column per record, each of shape `shape`.
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
    /// An offset and a size per record, in any order and possibly shared.
    Sized(AnySized<'a>),
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
            Self::Sized(AnySized::I32(s)) => s.len(),
            Self::Sized(AnySized::I64(s)) => s.len(),
            Self::Sized(AnySized::U32(s)) => s.len(),
            Self::Sized(AnySized::U64(s)) => s.len(),
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
            Self::Sized(AnySized::I32(s)) => s.elements.shape(),
            Self::Sized(AnySized::I64(s)) => s.elements.shape(),
            Self::Sized(AnySized::U32(s)) => s.elements.shape(),
            Self::Sized(AnySized::U64(s)) => s.elements.shape(),
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
