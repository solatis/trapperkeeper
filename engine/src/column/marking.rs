//! How a column marks the rows that have no value, and the loops that turn a
//! marking into the bitmap of a chunk. A source says how it marks "no value"
//! (ADR-0036), and a NaN is no value in every source (ADR-0005).

use super::values::{Bits, Strided};

/// Rows in one chunk, at most. The executor works on a column in chunks of
/// this size, which ADR-0035 sets.
pub const CHUNK_ROWS: usize = 2048;

/// Words of 64 bits in the bitmap of one chunk.
pub const CHUNK_WORDS: usize = CHUNK_ROWS / 64;

/// The bitmap of one chunk: row `i` of the chunk is bit `i % 64` of word
/// `i / 64`, and a set bit means that the row has a value.
// With this order, testing a row is one shift and one mask, a loop visits the
// set rows in order with `trailing_zeros`, and a logic step combines 64 rows
// per instruction.
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
