//! The properties of the column description. Each one builds a description
//! from generated data and checks that a reader or a loop gives that data
//! back. The properties keep proptest's default case count, so that
//! `PROPTEST_CASES` can lower it under Miri, and they generate smaller columns
//! under Miri.

// The half-precision column holds nightly's `f16` (ADR-0036).
#![feature(f16)]

use std::mem::size_of;

use proptest::collection::vec;
use proptest::prelude::*;
use proptest::sample::Index;
use trapperkeeper_engine::column::{
    Bits, CHUNK_ROWS, CHUNK_WORDS, Column, Float, Ints, Marking, Mismatch, Present, Strided,
    clear_nan, present_all, present_from_bits, present_from_mask,
};

/// The most rows of a generated column that a chunk is cut from. Natively it
/// exceeds two chunks, so a chunk of `CHUNK_ROWS` rows can start at any row
/// below `CHUNK_ROWS`. Miri interprets every row, so under Miri the columns
/// stay at 300 rows. That still covers several 64-row blocks, the rows after
/// the last block and unaligned starts, which are where a read out of bounds
/// would happen.
const MAX_ROWS: usize = if cfg!(miri) { 300 } else { 5000 };

/// Whether bit `row` of the bitmap of a chunk is set.
fn bit(out: &Present, row: usize) -> bool {
    out[row / 64] >> (row % 64) & 1 == 1
}

/// Checks that `out` is the bitmap of `expected`: one bit per row, and no bit
/// set from the end of `expected` on.
fn assert_bitmap(out: &Present, expected: &[bool]) -> Result<(), TestCaseError> {
    for (row, want) in expected.iter().enumerate() {
        prop_assert_eq!(bit(out, row), *want, "row {}", row);
    }
    // A later step combines whole words, so a stray bit past the chunk would
    // count as a row.
    prop_assert!((expected.len()..CHUNK_WORDS * 64).all(|row| !bit(out, row)));
    Ok(())
}

/// The bitmap of a chunk of `values` after its marking `present`, with the rows
/// whose float is a NaN cleared. Every `step`-th value of `values` is a row,
/// and `present` has one bit per row.
fn nan_cleared<T: Float>(
    values: &[T],
    step: usize,
    present: Bits<'_>,
    (start, rows): (usize, usize),
) -> Present {
    // SAFETY: the pointer is the slice's, the stride is `step` values in
    // bytes, and the count is the length of `present`, which the caller makes
    // the number of `step`-th values that the slice holds. So every value read
    // lies inside the slice.
    let run = unsafe { Strided::from_raw(values.as_ptr(), step * size_of::<T>(), present.len()) };
    // The bitmap starts full, so a word that the loops do not write shows.
    let mut out = [u64::MAX; CHUNK_WORDS];
    present_from_bits(present, start, rows, &mut out);
    clear_nan(run, start, rows, &mut out);
    out
}

/// The start and the length of a chunk of a column of `len` rows, picked by
/// two generated indexes. The first picks the start inside the column, and the
/// second picks a length of at most `CHUNK_ROWS` that stays inside it, so every
/// start is reached, also one that is no multiple of 64.
fn chunk_of(len: usize, (at, size): (Index, Index)) -> (usize, usize) {
    let start = at.index(len + 1);
    (start, size.index((len - start).min(CHUNK_ROWS) + 1))
}

/// Packs booleans into bytes from bit `offset` on, lowest bit first, as a
/// source bitmap holds them. The bits before `offset` are set, so a loop that
/// reads them shows.
fn packed(bools: &[bool], offset: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; (offset + bools.len()).div_ceil(8)];
    let bits = std::iter::repeat_n(true, offset).chain(bools.iter().copied());
    for (at, bit) in bits.enumerate() {
        bytes[at / 8] |= u8::from(bit) << (at % 8);
    }
    bytes
}

proptest! {
    // `failure_persistence: None` turns off proptest's regression file, so a
    // failing run writes no untracked file into the working tree.
    #![proptest_config(ProptestConfig {
        failure_persistence: None,
        .. ProptestConfig::default()
    })]

    /// A run reads back the values of the slice it was built from, and a
    /// strided run reads every `step`-th value.
    #[test]
    fn a_run_reads_back_its_values(xs in vec(any::<i64>(), 0..300), step in 1usize..4) {
        let run = Strided::from_slice(&xs);
        prop_assert_eq!(run.len(), xs.len());
        prop_assert!((0..xs.len()).all(|row| run.get(row) == xs[row]));
        // A run from a slice is contiguous.
        prop_assert_eq!(run.as_slice(), Some(&xs[..]));

        let every = xs.iter().step_by(step).copied().collect::<Vec<_>>();
        // SAFETY: the pointer is the slice's, the stride is `step` values in
        // bytes, and the count is the number of `step`-th values that the
        // slice holds, so every value read lies inside the slice.
        let strided = unsafe { Strided::from_raw(xs.as_ptr(), step * size_of::<i64>(), every.len()) };
        prop_assert!((0..every.len()).all(|row| strided.get(row) == every[row]));
        // A run with a step above 1 is not contiguous.
        prop_assert_eq!(strided.as_slice().is_some(), step == 1);
    }

    /// A run of bits reads back the booleans it was packed from, at any bit
    /// offset, and `Bits::new` refuses a buffer that is too short.
    #[test]
    fn bits_read_back_their_booleans(bools in vec(any::<bool>(), 0..300), offset in 0usize..17) {
        let bytes = packed(&bools, offset);
        let bits = Bits::new(&bytes, offset, bools.len()).expect("the buffer holds the bits");
        prop_assert_eq!(bits.len(), bools.len());
        prop_assert!((0..bools.len()).all(|row| bits.get(row) == bools[row]));
        // `packed` makes the shortest buffer, so one byte less cannot hold the
        // bits whenever there is a byte.
        if let Some(short) = bytes.len().checked_sub(1) {
            prop_assert!(Bits::new(&bytes[..short], offset, bools.len()).is_none());
        }
    }

    /// A column refuses a marking that covers another number of rows than its
    /// values, and takes one that covers the same number.
    #[test]
    fn a_marking_of_another_length_is_refused(values in 0usize..100, marking in 0usize..100) {
        let xs = vec![0i32; values];
        let mask = vec![0u8; marking];
        let ints = Ints::I32(Strided::from_slice(&xs));
        let column = Column::ints(ints, Marking::ByteMask(Strided::from_slice(&mask)));
        let expected = if values == marking { Ok(values) } else { Err(Mismatch { values, marking }) };
        prop_assert_eq!(column.map(|c| c.rows()), expected);
        // `Never` covers any number of rows.
        prop_assert_eq!(Column::ints(ints, Marking::Never).map(|c| c.rows()), Ok(values));
    }

    /// The bitmap of a chunk from a source bitmap has a bit per row of the
    /// chunk, at any bit offset and any start.
    #[test]
    fn a_chunk_of_a_bitmap_has_its_bits(
        bools in vec(any::<bool>(), 0..MAX_ROWS),
        offset in 0usize..17,
        cut in any::<(Index, Index)>(),
    ) {
        let bytes = packed(&bools, offset);
        let bits = Bits::new(&bytes, offset, bools.len()).expect("the buffer holds the bits");
        let (start, rows) = chunk_of(bools.len(), cut);
        // The bitmap starts full, so a word that the loop does not write shows.
        let mut out = [u64::MAX; CHUNK_WORDS];
        present_from_bits(bits, start, rows, &mut out);
        assert_bitmap(&out, &bools[start..start + rows])?;
    }

    /// The bitmap of a chunk from a byte mask has a bit where the byte is
    /// zero, for a contiguous mask and for a strided one.
    #[test]
    fn a_chunk_of_a_byte_mask_has_its_bits(
        // Half the bytes are zero, and the others are any byte, so a byte
        // such as 2 or 255 also counts as "no value".
        bytes in vec(prop_oneof![Just(0u8), any::<u8>()], 0..MAX_ROWS),
        step in 1usize..4,
        cut in any::<(Index, Index)>(),
    ) {
        let every = bytes.iter().step_by(step).copied().collect::<Vec<_>>();
        // SAFETY: the pointer is the vector's, the stride is `step` bytes, and
        // the count is the number of `step`-th bytes that the vector holds, so
        // every byte read lies inside it.
        let mask = unsafe { Strided::from_raw(bytes.as_ptr(), step, every.len()) };
        let (start, rows) = chunk_of(every.len(), cut);
        let mut out = [u64::MAX; CHUNK_WORDS];
        present_from_mask(mask, start, rows, &mut out);
        let expected = every[start..start + rows].iter().map(|b| *b == 0).collect::<Vec<_>>();
        assert_bitmap(&out, &expected)?;
    }

    /// After a bitmap marking, `clear_nan` leaves a bit set only where the
    /// marking says present and the float is not a NaN, for f16, f32 and f64,
    /// contiguous and strided.
    #[test]
    fn a_nan_is_cleared_after_the_marking(
        xs in vec(prop_oneof![Just(f64::NAN), any::<f64>()], 0..MAX_ROWS),
        // The mask is as long as the longest column, so it covers the values
        // whatever their number, and the property cuts it to that number.
        mask in vec(any::<bool>(), MAX_ROWS),
        step in 1usize..4,
        cut in any::<(Index, Index)>(),
    ) {
        let every = xs.iter().step_by(step).copied().collect::<Vec<_>>();
        let marked = &mask[..every.len()];
        let bytes = packed(marked, 0);
        let present = Bits::new(&bytes, 0, every.len()).expect("the buffer holds the bits");
        let chunk @ (start, rows) = chunk_of(every.len(), cut);
        // A row the marking calls missing stays missing, and a row the marking
        // calls present goes missing where its float is a NaN.
        let expected = (start..start + rows).map(|r| marked[r] && !every[r].is_nan()).collect::<Vec<_>>();
        assert_bitmap(&nan_cleared(&xs, step, present, chunk), &expected)?;
        // Narrowing keeps a NaN a NaN and turns no other value into one, so
        // the three widths share one expectation. A large f64 narrows to an
        // infinity, which is no NaN.
        #[expect(clippy::cast_possible_truncation, reason = "the property narrows on purpose")]
        let narrow = xs.iter().map(|x| *x as f32).collect::<Vec<_>>();
        assert_bitmap(&nan_cleared(&narrow, step, present, chunk), &expected)?;
        // Clippy's `cast_possible_truncation` does not check a cast to `f16`,
        // so this cast needs no `expect`.
        let half = xs.iter().map(|x| *x as f16).collect::<Vec<_>>();
        assert_bitmap(&nan_cleared(&half, step, present, chunk), &expected)?;
    }

    /// The bitmap of a chunk of a column that marks nothing has every row of
    /// the chunk.
    #[test]
    fn a_chunk_without_a_marking_has_every_row(rows in 0..=CHUNK_ROWS) {
        // The bitmap starts full, so a word that the loop does not clear shows.
        let mut out = [u64::MAX; CHUNK_WORDS];
        present_all(rows, &mut out);
        assert_bitmap(&out, &vec![true; rows])?;
    }
}

#[test]
#[should_panic(expected = "a chunk of 2049 rows")]
fn a_chunk_above_the_chunk_size_panics() {
    // 300 bytes hold the 2400 bits, which are more than a chunk, so only the
    // bound on the chunk size fails.
    let bytes = [0u8; 300];
    let bits = Bits::new(&bytes, 0, 2400).expect("the buffer holds the bits");
    present_from_bits(bits, 0, CHUNK_ROWS + 1, &mut [0; CHUNK_WORDS]);
}

#[test]
#[should_panic(expected = "a chunk of 10 rows from row 95 of 100")]
fn a_chunk_past_the_column_panics() {
    let bytes = [0u8; 13];
    let bits = Bits::new(&bytes, 0, 100).expect("the buffer holds the bits");
    present_from_bits(bits, 95, 10, &mut [0; CHUNK_WORDS]);
}
