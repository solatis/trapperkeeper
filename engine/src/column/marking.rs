//! How a column marks the rows that have no value, and the loops that turn a
//! marking into the bitmap of a chunk. A source says how it marks "no value"
//! (ADR-0036), and a NaN is no value in every source (ADR-0005).

use std::simd::cmp::{SimdPartialEq, SimdPartialOrd};
use std::simd::num::SimdFloat;
use std::simd::{Simd, SimdElement};

use super::values::{Bits, Strided};

/// Rows in one chunk, at most. The executor works on a column in chunks of
/// this size, which ADR-0035 sets. No measurement has fixed the number yet.
pub const CHUNK_ROWS: usize = 2048;

/// Words of 64 bits in the bitmap of one chunk.
pub const CHUNK_WORDS: usize = CHUNK_ROWS / 64;

/// The bitmap of one chunk: row `i` of the chunk is bit `i % 64` of word
/// `i / 64`, and a set bit means that the row has a value.
// With this order, testing a row is one shift and one mask, a loop visits the
// set rows in order with `trailing_zeros`, and a logic step combines 64 rows
// per instruction. A source bitmap in the same bit order, such as Arrow's,
// copies with shifts alone.
pub type Present = [u64; CHUNK_WORDS];

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

/// Writes the bitmap of a chunk of `rows` rows that all have a value, for a
/// column whose marking is `Never`. The bits from `rows` on are 0.
///
/// # Panics
///
/// Panics when `rows` is above `CHUNK_ROWS`.
pub fn present_all(rows: usize, out: &mut Present) {
    assert!(rows <= CHUNK_ROWS, "a chunk of {rows} rows");
    for (k, word) in out.iter_mut().enumerate() {
        // Word `k` holds the rows from `64 * k` on: all 64 of them, the
        // `rows % 64` rows of a last part-filled word, or none.
        let held = rows.saturating_sub(64 * k).min(64);
        // A full word takes its own branch, because `1 << 64` overflows a
        // `u64`.
        *word = if held == 64 {
            u64::MAX
        } else {
            (1 << held) - 1
        };
    }
}

/// The 64 bits of `bytes` from bit `bit` on, lowest first. A bit past the end
/// of `bytes` reads as 0.
fn word_at(bytes: &[u8], bit: usize) -> u64 {
    // 64 bits that start inside a byte reach into a ninth byte. The buffer is
    // zero where `bytes` has ended.
    let mut buf = [0u8; 9];
    // `get` gives nothing for a position past the end, so the word stays 0
    // and no index panics.
    if let Some(src) = bytes.get(bit / 8..) {
        let n = src.len().min(buf.len());
        buf[..n].copy_from_slice(&src[..n]);
    }
    let [low @ .., high] = buf;
    let low = u64::from_le_bytes(low);
    let shift = bit % 8;
    // At a byte boundary the low eight bytes are the whole word, and the high
    // byte would be shifted by 64, which overflows a `u64`.
    if shift == 0 {
        low
    } else {
        low >> shift | u64::from(high) << (64 - shift)
    }
}

/// Writes the bitmap of the rows `start .. start + rows` of a source bitmap
/// into `out`. Every word of `out` is written, and the bits from `rows` on are
/// 0.
///
/// # Panics
///
/// Panics when `rows` is above `CHUNK_ROWS` or the chunk reaches past `bits`.
pub fn present_from_bits(bits: Bits<'_>, start: usize, rows: usize, out: &mut Present) {
    // The bound keeps the chunk inside `out` and inside `bits`.
    assert!(
        rows <= CHUNK_ROWS && start.checked_add(rows).is_some_and(|end| end <= bits.len()),
        "a chunk of {rows} rows from row {start} of {}",
        bits.len()
    );
    let (bytes, offset) = bits.parts();
    for (k, word) in out.iter_mut().enumerate() {
        // Word `k` holds the 64 source bits from bit `offset + start + 64 * k`
        // on, and a word that starts at or past `rows` holds no row.
        *word = if 64 * k < rows {
            word_at(bytes, offset + start + 64 * k)
        } else {
            0
        };
    }
    // The last word that holds rows may also hold bits of the rows after the
    // chunk. The mask clears them, because a later step counts and combines
    // whole words.
    if !rows.is_multiple_of(64) {
        out[rows / 64] &= (1 << (rows % 64)) - 1;
    }
}

/// Where `pack` reads the rows of a chunk.
trait Source: Copy {
    /// 64 rows of the source, as `Test::lanes` takes them.
    type Lanes;
    /// One row of the source, as `Test::one` takes it.
    type One;

    /// The number of rows.
    fn len(&self) -> usize;

    /// Whether the values lie one after the other, so that `lanes` can load
    /// them.
    fn is_contiguous(&self) -> bool;

    /// The 64 rows from `row` on.
    ///
    /// # Panics
    ///
    /// Panics when the values do not lie one after the other or the rows reach
    /// past the end.
    fn lanes(&self, row: usize) -> Self::Lanes;

    /// The row `row`, without the bound check.
    ///
    /// # Safety
    ///
    /// `row` is below `len`.
    unsafe fn one(&self, row: usize) -> Self::One;
}

impl<T: SimdElement> Source for Strided<'_, T> {
    type Lanes = Simd<T, 64>;
    type One = T;

    fn len(&self) -> usize {
        Strided::len(self)
    }

    fn is_contiguous(&self) -> bool {
        self.as_slice().is_some()
    }

    fn lanes(&self, row: usize) -> Simd<T, 64> {
        let values = self.as_slice().expect("the values lie one after the other");
        Simd::from_slice(&values[row..row + 64])
    }

    unsafe fn one(&self, row: usize) -> T {
        // SAFETY: the caller keeps `row` below `len`.
        unsafe { self.get_unchecked(row) }
    }
}

/// A test that `pack` applies to each row of a chunk.
// The two methods give the same answer for the same row, so the vector arm
// and the scalar arm of `pack` agree.
trait Test<S: Source> {
    /// The answers for 64 rows, with the answer for lane `i` in bit `i`.
    fn lanes(&self, rows: S::Lanes) -> u64;
    /// The answer for one row.
    fn one(&self, row: S::One) -> bool;
}

/// The test of a byte mask: the byte is zero, so the row has a value.
struct ZeroByte;

impl Test<Strided<'_, u8>> for ZeroByte {
    fn lanes(&self, values: Simd<u8, 64>) -> u64 {
        values.simd_eq(Simd::splat(0)).to_bitmask()
    }

    fn one(&self, value: u8) -> bool {
        value == 0
    }
}

/// Writes into `out` the bitmap of the rows `start .. start + rows` of
/// `source`, with a bit set where `test` says so. Every word of `out` is
/// written, and the bits from `rows` on are 0.
///
/// # Panics
///
/// Panics when `rows` is above `CHUNK_ROWS` or the chunk reaches past the end
/// of `source`.
fn pack<S: Source, X: Test<S>>(source: S, start: usize, rows: usize, out: &mut Present, test: &X) {
    // The bound keeps the chunk inside `out`, and the unchecked reads of the
    // scalar loop trust it.
    assert!(
        rows <= CHUNK_ROWS
            && start
                .checked_add(rows)
                .is_some_and(|end| end <= source.len()),
        "a chunk of {rows} rows from row {start} of {}",
        source.len()
    );
    // The loops below write only the words that hold rows.
    out.fill(0);
    // Values that lie one after the other load 64 at a time into a vector,
    // and `Test::lanes` gives one word per load. The rows after the last full
    // group of 64 are left to the scalar loop, so no load reads past the
    // chunk. A strided source reads every row in the scalar loop, because its
    // values are not adjacent in memory.
    let first = if source.is_contiguous() {
        for (k, word) in out.iter_mut().take(rows / 64).enumerate() {
            *word = test.lanes(source.lanes(start + 64 * k));
        }
        rows / 64 * 64
    } else {
        0
    };
    for row in first..rows {
        // SAFETY: the assertion above keeps `start + row` below `len`.
        let value = unsafe { source.one(start + row) };
        set_if(out, row, test.one(value));
    }
}

/// Sets the bit of `row` in `out` when `on` is true.
fn set_if(out: &mut Present, row: usize, on: bool) {
    out[row / 64] |= u64::from(on) << (row % 64);
}

/// Writes the bitmap of the rows `start .. start + rows` of a byte mask into
/// `out`: a row has a value where its byte is zero.
///
/// # Panics
///
/// Panics when `rows` is above `CHUNK_ROWS` or the chunk reaches past `mask`.
pub fn present_from_mask(mask: Strided<'_, u8>, start: usize, rows: usize, out: &mut Present) {
    pack(mask, start, rows, out, &ZeroByte);
}

/// The test of a float: the value is not a NaN, so the row keeps its value.
struct NotNan;

impl Test<Strided<'_, f32>> for NotNan {
    fn lanes(&self, values: Simd<f32, 64>) -> u64 {
        !values.is_nan().to_bitmask()
    }

    fn one(&self, value: f32) -> bool {
        !value.is_nan()
    }
}

impl Test<Strided<'_, f64>> for NotNan {
    fn lanes(&self, values: Simd<f64, 64>) -> u64 {
        !values.is_nan().to_bitmask()
    }

    fn one(&self, value: f64) -> bool {
        !value.is_nan()
    }
}

/// The bits of positive infinity in half precision. A half-precision value is
/// a NaN when its bits without the sign lie above these.
const F16_INFINITY: u16 = f16::INFINITY.to_bits();

// The test of a half-precision float runs on its 16 bits as ints. Every
// target tests the bits without widening a lane, which aarch64 without its
// half-precision extension does for a test of `f16` lanes.
impl Test<Strided<'_, u16>> for NotNan {
    fn lanes(&self, values: Simd<u16, 64>) -> u64 {
        // `0x7fff` clears the sign bit, so a negative NaN tests like a
        // positive one.
        (values & Simd::splat(0x7fff))
            .simd_le(Simd::splat(F16_INFINITY))
            .to_bitmask()
    }

    fn one(&self, value: u16) -> bool {
        // `0x7fff` clears the sign bit, as in `lanes`.
        (value & 0x7fff) <= F16_INFINITY
    }
}

mod sealed {
    use super::{Present, Strided};

    /// The part of `Float` that only this crate can name, so no crate outside
    /// the engine implements `Float` for a type of its own.
    pub trait Sealed: Copy + Sized {
        /// Writes into `out` the bitmap of the rows of the chunk whose float is
        /// not a NaN.
        fn not_nan(values: Strided<'_, Self>, start: usize, rows: usize, out: &mut Present);
    }
}

/// A float type that a column can hold.
// The trait is sealed, because the engine owns the closed list of float types
// and no other crate extends it (ADR-0036).
pub trait Float: sealed::Sealed {}

impl sealed::Sealed for f16 {
    fn not_nan(values: Strided<'_, Self>, start: usize, rows: usize, out: &mut Present) {
        // SAFETY: a `u16` has the size and the alignment of an `f16`, and
        // every bit pattern is a valid `u16`.
        let bits = unsafe { values.cast::<u16>() };
        pack(bits, start, rows, out, &NotNan);
    }
}

impl sealed::Sealed for f32 {
    fn not_nan(values: Strided<'_, Self>, start: usize, rows: usize, out: &mut Present) {
        pack(values, start, rows, out, &NotNan);
    }
}

impl sealed::Sealed for f64 {
    fn not_nan(values: Strided<'_, Self>, start: usize, rows: usize, out: &mut Present) {
        pack(values, start, rows, out, &NotNan);
    }
}

impl Float for f16 {}
impl Float for f32 {}
impl Float for f64 {}

/// Clears in `out` the bit of every row of the chunk whose float is a NaN,
/// because a NaN is a null (ADR-0005). Bit `i` of `out` is row `start + i` of
/// the column, and the bits from `rows` on become 0. The loop of the column's
/// marking writes `out` first, and this pass runs after it.
///
/// # Panics
///
/// Panics when `rows` is above `CHUNK_ROWS` or the chunk reaches past
/// `values`.
pub fn clear_nan<T: Float>(values: Strided<'_, T>, start: usize, rows: usize, out: &mut Present) {
    let mut not_nan = [0; CHUNK_WORDS];
    T::not_nan(values, start, rows, &mut not_nan);
    // The pass only clears bits, so a row that the marking counts as missing
    // stays missing.
    for (word, keep) in out.iter_mut().zip(not_nan) {
        *word &= keep;
    }
}
