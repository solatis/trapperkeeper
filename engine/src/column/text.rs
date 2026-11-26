//! The layouts of a text column and the trait `Texts` through which a loop
//! reads any of them. Nothing in this file copies a text.

use std::marker::PhantomData;
use std::ops::Range;
use std::ptr::NonNull;
use std::slice;

use super::layout::{self, AnyOffsets, Fixed, Offset, Offsets, Storage};
use super::values::Strided;

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

/// Bytes of text that a view holds itself, at most.
// A view has 16 bytes, and its length takes the first four.
pub const INLINE: usize = 12;

/// One view of a text: its length, and then either the text itself, when it
/// has at most `INLINE` bytes, or the first four bytes of the text, the number
/// of the buffer that holds it and its offset in that buffer. Polars and
/// `DuckDB` hold their texts as views, and so does Arrow data.
// The alignment is 4, because the widest field is 4 bytes. An alignment of 16
// would refuse memory that allocators align only to 8.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct View {
    len: u32,
    // Bytes 4 to 15 of the view. A short text lies here, padded with zeros. For
    // a long text the bytes hold its first four bytes, then the buffer number,
    // then the offset, four bytes each.
    rest: [u8; 12],
}

// Sources hand over views of 16 bytes, so the type must keep that size.
const _: () = assert!(size_of::<View>() == 16);

/// The largest length, buffer number or offset of a view. The three numbers
/// are signed 32-bit integers in the Arrow format.
const VIEW_MAX: usize = i32::MAX as usize;

impl View {
    /// The view of a text of at most `INLINE` bytes. `None` for a longer
    /// text.
    #[must_use]
    pub fn inline(text: &str) -> Option<Self> {
        let bytes = text.as_bytes();
        let len = u32::try_from(bytes.len())
            .ok()
            .filter(|n| *n as usize <= INLINE)?;
        let mut rest = [0; 12];
        rest[..bytes.len()].copy_from_slice(bytes);
        Some(Self { len, rest })
    }

    /// The view of a text of more than `INLINE` bytes that lies in buffer
    /// `buffer` at `offset`. `None` for a shorter text, or when a number does
    /// not fit 31 bits.
    #[must_use]
    pub fn outside(text: &str, buffer: u32, offset: u32) -> Option<Self> {
        let bytes = text.as_bytes();
        let len = u32::try_from(bytes.len()).ok()?;
        // The numbers of a view are signed, so a number that needs the 32nd
        // bit has no view.
        let fits = |n: u32| n as usize <= VIEW_MAX;
        if bytes.len() <= INLINE || !fits(len) || !fits(buffer) || !fits(offset) {
            return None;
        }
        let mut rest = [0; 12];
        // The first four bytes of the text, then the buffer number and the
        // offset, in the byte order of the machine.
        rest[..4].copy_from_slice(&bytes[..4]);
        rest[4..8].copy_from_slice(&buffer.to_ne_bytes());
        rest[8..].copy_from_slice(&offset.to_ne_bytes());
        Some(Self { len, rest })
    }

    /// The length of the text in bytes.
    fn len(&self) -> usize {
        self.len as usize
    }

    /// The buffer number and the offset of a long text.
    fn place(&self) -> (usize, usize) {
        // Bytes 4 to 7 of `rest` are the buffer number and bytes 8 to 11 the
        // offset, after the first four bytes of the text.
        let number = |at: usize| {
            let mut word = [0; 4];
            word.copy_from_slice(&self.rest[at..at + 4]);
            u32::from_ne_bytes(word) as usize
        };
        (number(4), number(8))
    }
}

/// A text column of views, with the buffers that the long texts lie in.
#[derive(Debug, Clone, Copy)]
pub struct Views<'a> {
    views: &'a [View],
    // One slice per buffer of the source, often one or two per batch, which
    // the adapter lends.
    buffers: &'a [&'a [u8]],
}

impl<'a> Views<'a> {
    /// A column of one text per view.
    ///
    /// # Safety
    ///
    /// A view with a length above `INLINE` names a buffer of `buffers`, and
    /// its offset plus its length is at most the length of that buffer. The
    /// bytes of every text are UTF-8.
    #[must_use]
    pub unsafe fn new(views: &'a [View], buffers: &'a [&'a [u8]]) -> Self {
        // This checks the whole contract, so that a caller that breaks it
        // fails in a debug build and under Miri.
        debug_assert!(
            Self::valid(views, buffers),
            "the views break the contract of `new`"
        );
        Self { views, buffers }
    }

    /// Whether the views and the buffers keep the contract of `new`.
    fn valid(views: &[View], buffers: &[&[u8]]) -> bool {
        views.iter().all(|v| {
            let len = v.len();
            let bytes = if len <= INLINE {
                // Only the first `len` of the twelve bytes are text.
                Some(&v.rest[..len])
            } else {
                let (number, offset) = v.place();
                buffers
                    .get(number)
                    .and_then(|b| b.get(offset..offset.checked_add(len)?))
            };
            bytes.is_some_and(|b| str::from_utf8(b).is_ok())
        })
    }
}

impl sealed::Sealed for Views<'_> {}

impl<'a> Texts<'a> for Views<'a> {
    type Unit = u8;

    fn len(&self) -> usize {
        self.views.len()
    }

    fn get(&self, row: usize) -> &'a [u8] {
        let view = &self.views[row];
        let len = view.len();
        // A text of at most `INLINE` bytes lies in the view, and a longer one
        // in the buffer that the view names.
        if len <= INLINE {
            return &view.rest[..len];
        }
        let (number, offset) = view.place();
        // SAFETY: the contract of `Views::new` keeps the buffer number, the
        // offset and the length inside `buffers`.
        unsafe {
            self.buffers
                .get_unchecked(number)
                .get_unchecked(offset..offset + len)
        }
    }
}

/// The address and the length of the bytes of one text, which stays where the
/// source holds it. An adapter over a slice of Rust structs fills a run of
/// pieces from `String` or `&str` fields and copies no text.
// `repr(C)` fixes the layout, because Rust does not fix the layout of a `&str`,
// and an adapter in another language writes pieces too.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Piece<'a> {
    first: NonNull<u8>,
    len: usize,
    borrow: PhantomData<&'a str>,
}

impl<'a> Piece<'a> {
    /// The piece of a text.
    #[must_use]
    pub fn new(text: &'a str) -> Self {
        Self {
            first: NonNull::from(text.as_bytes()).cast(),
            len: text.len(),
            borrow: PhantomData,
        }
    }

    /// A piece over bytes that Rust did not allocate.
    ///
    /// # Safety
    ///
    /// `first` is not null, `len` bytes are readable from it, and nothing
    /// writes them during `'a`. The bytes are UTF-8.
    #[must_use]
    pub unsafe fn from_raw(first: *const u8, len: usize) -> Self {
        debug_assert!(!first.is_null(), "the text is at a null address");
        // SAFETY: the caller promises that `len` bytes are readable from
        // `first`. A debug build reads them once to check the UTF-8 promise.
        let bytes = unsafe { slice::from_raw_parts(first, len) };
        debug_assert!(str::from_utf8(bytes).is_ok(), "the text is not UTF-8");
        Self {
            // SAFETY: the caller promises that `first` is not null.
            first: unsafe { NonNull::new_unchecked(first.cast_mut()) },
            len,
            borrow: PhantomData,
        }
    }

    /// The bytes of the text.
    fn bytes(&self) -> &'a [u8] {
        // SAFETY: both constructors make `len` bytes readable from `first`
        // for `'a`.
        unsafe { slice::from_raw_parts(self.first.as_ptr(), self.len) }
    }
}

impl sealed::Sealed for Strided<'_, Piece<'_>> {}

impl<'a> Texts<'a> for Strided<'a, Piece<'a>> {
    type Unit = u8;

    fn len(&self) -> usize {
        Strided::len(self)
    }

    fn get(&self, row: usize) -> &'a [u8] {
        Strided::get(self, row).bytes()
    }
}

/// The units of numpy cells of one width, end to end. A cell's text ends
/// before its trailing zero units.
#[derive(Debug, Clone, Copy)]
pub struct Units<'a, U>(&'a [U]);

impl<'a> Units<'a, u8> {
    /// The bytes of numpy's `S` cells.
    ///
    /// # Safety
    ///
    /// Every row that a fixed-width layout over these bytes names holds UTF-8
    /// up to its trailing zeros. `Fixed::new` checks it in a debug build.
    #[must_use]
    pub unsafe fn utf8(bytes: &'a [u8]) -> Self {
        Self(bytes)
    }
}

impl<'a> Units<'a, u32> {
    /// The units of numpy's `U` cells, in the byte order of the machine.
    // The constructor is safe, because the reader returns units and promises
    // no valid characters, so no value of a unit is wrong. A Python text can
    // hold a lone surrogate, which a `char` must not hold.
    #[must_use]
    pub fn utf32(units: &'a [u32]) -> Self {
        Self(units)
    }
}

impl<U> layout::sealed::Sealed for Units<'_, U> {}

impl Storage for Units<'_, u8> {
    fn count(&self) -> usize {
        self.0.len()
    }

    fn holds(&self, range: Range<usize>) -> bool {
        self.0
            .get(range)
            .is_some_and(|cell| str::from_utf8(trim(cell)).is_ok())
    }
}

impl Storage for Units<'_, u32> {
    fn count(&self) -> usize {
        self.0.len()
    }

    fn holds(&self, _: Range<usize>) -> bool {
        true
    }
}

/// The text of a cell: the cell without its trailing zero units.
fn trim<U: Unit>(cell: &[U]) -> &[U] {
    // The text ends after the last unit that is not zero, so a cell of zeros
    // alone is the empty text. A text that itself ends in a zero unit loses
    // it, as it does in numpy.
    let end = cell
        .iter()
        .rposition(|u| *u != U::ZERO)
        .map_or(0, |i| i + 1);
    &cell[..end]
}

impl<U: Unit> sealed::Sealed for Fixed<Units<'_, U>> {}

impl<'a, U: Unit> Texts<'a> for Fixed<Units<'a, U>>
where
    Units<'a, U>: Storage,
{
    type Unit = U;

    fn len(&self) -> usize {
        Fixed::len(self)
    }

    fn get(&self, row: usize) -> &'a [U] {
        trim(&self.storage().0[self.range(row)])
    }
}

/// The layout of a text column.
#[derive(Debug, Clone, Copy)]
pub enum Text<'a> {
    /// Texts end to end in one buffer, with offsets of any of the four types,
    /// as Polars and `DuckDB` hold them.
    Offsets(AnyOffsets<'a, Utf8<'a>>),
    /// Views of 16 bytes.
    Views(Views<'a>),
    /// A pointer and a length per row, as an adapter over a slice of Rust
    /// structs fills them.
    Pieces(Strided<'a, Piece<'a>>),
    /// Fixed-width cells of UTF-8 bytes, as numpy's `S` type.
    Bytes(Fixed<Units<'a, u8>>),
    /// Fixed-width cells of UTF-32, as numpy's `U` type. This is the one
    /// layout that is not UTF-8 (ADR-0041).
    Utf32(Fixed<Units<'a, u32>>),
}

impl Text<'_> {
    /// The number of texts.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::Offsets(o) => o.len(),
            Self::Views(v) => Texts::len(v),
            Self::Pieces(p) => Strided::len(p),
            Self::Bytes(f) => f.len(),
            Self::Utf32(f) => f.len(),
        }
    }

    /// Whether the column holds no text.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
