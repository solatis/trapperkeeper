//! The layouts that text and list columns share. Each says which elements
//! belong to a row, and is generic over what the elements are: the bytes of
//! texts, the units of numpy cells, or the element column of a list.

use std::ops::Range;

pub(super) mod sealed {
    /// The part of the traits of this module that only the engine can name,
    /// so no other crate implements them.
    pub trait Sealed {}

    impl Sealed for i32 {}
    impl Sealed for i64 {}
    impl Sealed for u32 {}
    impl Sealed for u64 {}
}

/// An offset type of an offsets layout: `i32`, `i64`, `u32` or `u64`. More
/// types are added when a source needs them.
// The trait is sealed, because the engine owns the closed list of cases
// (ADR-0036).
pub trait Offset: Copy + sealed::Sealed {
    /// The offset as an index. The caller knows that it is not negative and
    /// fits a `usize`.
    fn index(self) -> usize;
    /// Whether the offset is not negative and fits a `usize`.
    fn fits(self) -> bool;
}

impl Offset for i32 {
    #[expect(
        clippy::cast_sign_loss,
        reason = "the caller knows the offset is not negative"
    )]
    fn index(self) -> usize {
        // The layout's contract says that the offset fits, so the cast loses
        // nothing.
        self as usize
    }

    fn fits(self) -> bool {
        usize::try_from(self).is_ok()
    }
}

impl Offset for i64 {
    #[expect(
        clippy::cast_sign_loss,
        clippy::cast_possible_truncation,
        reason = "the caller knows the offset fits a usize"
    )]
    fn index(self) -> usize {
        // The layout's contract says that the offset fits, so the cast loses
        // nothing.
        self as usize
    }

    fn fits(self) -> bool {
        usize::try_from(self).is_ok()
    }
}

impl Offset for u32 {
    fn index(self) -> usize {
        // The layout's contract says that the offset fits, so the cast loses
        // nothing.
        self as usize
    }

    fn fits(self) -> bool {
        usize::try_from(self).is_ok()
    }
}

impl Offset for u64 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the caller knows the offset fits a usize"
    )]
    fn index(self) -> usize {
        // The layout's contract says that the offset fits, so the cast loses
        // nothing.
        self as usize
    }

    fn fits(self) -> bool {
        usize::try_from(self).is_ok()
    }
}

/// What the rows of a shared layout point into.
// Each implementation says how many elements there are and what every range
// that a layout names must hold, such as UTF-8 for the bytes of texts. The
// trait is sealed, because the engine owns the closed list of cases
// (ADR-0036).
pub trait Storage: Copy + sealed::Sealed {
    /// The number of elements.
    fn count(&self) -> usize;
    /// Whether the elements in `range` hold what this type requires. A debug
    /// build checks it for every range that a layout names.
    fn holds(&self, range: Range<usize>) -> bool;
}

/// A layout in which row `i` holds the elements from `offsets[i]` to
/// `offsets[i + 1]`, so the elements of all rows lie end to end. The first
/// offset need not be 0, as in a slice of a longer column.
#[derive(Debug, Clone, Copy)]
pub struct Offsets<'a, O, S> {
    offsets: &'a [O],
    storage: S,
}

impl<'a, O: Offset, S: Storage> Offsets<'a, O, S> {
    /// A layout of `offsets.len() - 1` rows over `storage`.
    ///
    /// # Safety
    ///
    /// `offsets` holds at least one offset. Every offset fits a `usize`, no
    /// offset is smaller than the one before it, and the last is at most
    /// `storage.count()`. Every range between two neighbouring offsets holds
    /// what `S` requires.
    #[must_use]
    pub unsafe fn new(offsets: &'a [O], storage: S) -> Self {
        // This checks the whole contract, so that a caller that breaks it
        // fails in a debug build and under Miri.
        debug_assert!(
            Self::valid(offsets, &storage),
            "the offsets break the contract of `new`"
        );
        Self { offsets, storage }
    }

    /// The number of rows.
    #[must_use]
    pub fn len(&self) -> usize {
        // `n` rows have `n + 1` offsets, and the contract makes the slice
        // non-empty.
        self.offsets.len() - 1
    }

    /// Whether the layout has no row.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The elements of the row at `row`.
    ///
    /// # Panics
    ///
    /// Panics when `row` is not below `len`.
    #[must_use]
    pub fn range(&self, row: usize) -> Range<usize> {
        self.offsets[row].index()..self.offsets[row + 1].index()
    }

    /// What the rows point into.
    #[must_use]
    pub fn storage(&self) -> S {
        self.storage
    }

    /// Whether the offsets keep the contract of `new` over `storage`.
    fn valid(offsets: &[O], storage: &S) -> bool {
        // `fits` runs before any `index`, because a negative offset would
        // cast to a huge index.
        !offsets.is_empty()
            && offsets.iter().all(|o| o.fits())
            && offsets.last().is_some_and(|o| o.index() <= storage.count())
            // Each pair of neighbouring offsets is one row, which must not
            // run backwards and must hold what `S` requires.
            && offsets.windows(2).all(|w| {
                let (start, end) = (w[0].index(), w[1].index());
                start <= end && storage.holds(start..end)
            })
    }
}

/// An offsets layout with any of the four offset types.
#[derive(Debug, Clone, Copy)]
pub enum AnyOffsets<'a, S> {
    I32(Offsets<'a, i32, S>),
    I64(Offsets<'a, i64, S>),
    U32(Offsets<'a, u32, S>),
    U64(Offsets<'a, u64, S>),
}

impl<S: Storage> AnyOffsets<'_, S> {
    /// The number of rows.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::I32(o) => o.len(),
            Self::I64(o) => o.len(),
            Self::U32(o) => o.len(),
            Self::U64(o) => o.len(),
        }
    }

    /// Whether the layout has no row.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A layout in which every row holds `size` elements, and row `i` starts at
/// element `i * step`. `step` equals `size` when the rows lie one after the
/// other, and is larger for a stepped view.
#[derive(Debug, Clone, Copy)]
pub struct Fixed<S> {
    size: usize,
    step: usize,
    // With a size of 0 the number of rows cannot be derived from the
    // elements, so the caller states it.
    len: usize,
    storage: S,
}

impl<S: Storage> Fixed<S> {
    /// A layout of `len` rows of `size` elements, `step` elements apart.
    /// `None` when `step` is below `size` or the last row reaches past the
    /// elements.
    #[must_use]
    pub fn new(size: usize, step: usize, len: usize, storage: S) -> Option<Self> {
        // The end of the last row, `(len - 1) * step + size`, uses checked
        // arithmetic, so a count that overflows is refused and does not wrap.
        // With this guard no row reaches past the elements, which is why the
        // constructor is safe.
        let end = match len.checked_sub(1) {
            None => 0,
            Some(last) => last.checked_mul(step)?.checked_add(size)?,
        };
        if step < size || end > storage.count() {
            return None;
        }
        let layout = Self {
            size,
            step,
            len,
            storage,
        };
        // The storage's own promise, such as UTF-8 in every cell, is checked
        // in a debug build and under Miri.
        debug_assert!(
            (0..len).all(|row| storage.holds(layout.range(row))),
            "a row breaks the promise of the storage"
        );
        Some(layout)
    }

    /// The number of rows.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the layout has no row.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The elements of the row at `row`.
    ///
    /// # Panics
    ///
    /// Panics when `row` is not below `len`.
    #[must_use]
    pub fn range(&self, row: usize) -> Range<usize> {
        assert!(row < self.len, "row {row} of a layout of {}", self.len);
        let start = row * self.step;
        start..start + self.size
    }

    /// What the rows point into.
    #[must_use]
    pub fn storage(&self) -> S {
        self.storage
    }
}
