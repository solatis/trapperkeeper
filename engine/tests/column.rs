//! The properties of the column description. Each one builds a description
//! from generated data and checks that a reader or a loop gives that data
//! back. The properties keep proptest's default case count, so that
//! `PROPTEST_CASES` can lower it under Miri.

// The half-precision column holds nightly's `f16` (ADR-0036).
#![feature(f16)]

use std::mem::size_of;

use proptest::collection::vec;
use proptest::prelude::*;
use proptest::sample::Index;
use trapperkeeper_engine::column::{
    AnyOffsets, Bits, CHUNK_ROWS, CHUNK_WORDS, Column, Fixed, Float, INLINE, Ints, Marking,
    Mismatch, Offsets, Piece, Present, Strided, Text, Texts, Units, Utf8, View, Views, clear_nan,
    present_all, present_from_bits, present_from_mask,
};

/// The most rows of a generated column that a chunk is cut from. It exceeds
/// two chunks, so a chunk of `CHUNK_ROWS` rows can start at any row below
/// `CHUNK_ROWS`.
const MAX_ROWS: usize = 5000;

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

/// Texts for the layouts: empty texts, texts of one to four bytes per
/// character, and texts on both sides of the 12 bytes that a view holds
/// itself. No text holds U+0000, which a fixed-width cell cannot keep at the
/// end of a value.
fn texts() -> impl Strategy<Value = Vec<String>> {
    // The class draws ASCII without U+0000 and one character each of two,
    // three and four bytes, so the ends of the texts fall after every width
    // of character. Up to 30 characters put texts below, at and above 12
    // bytes.
    vec("[\\x01-\\x7f\u{e9}\u{20ac}\u{1f600}]{0,30}", 0..40)
}

/// Checks that every row of `layout` gives the bytes of `expected`, as UTF-8
/// that reads back to the same text.
fn assert_texts<'a, T: Texts<'a, Unit = u8>>(
    layout: T,
    expected: &[String],
) -> Result<(), TestCaseError> {
    prop_assert_eq!(layout.len(), expected.len());
    for (row, text) in expected.iter().enumerate() {
        prop_assert_eq!(
            str::from_utf8(layout.get(row)),
            Ok(text.as_str()),
            "row {}",
            row
        );
    }
    Ok(())
}

/// The offsets of `ends` in the offset type `O`.
fn offsets_of<O: TryFrom<usize>>(ends: &[usize]) -> Vec<O> {
    ends.iter()
        .map(|e| {
            // `expect` needs a `Debug` error, which `O::Error` need not be,
            // so the error is dropped.
            O::try_from(*e)
                .map_err(|_| ())
                .expect("every end fits the offset type")
        })
        .collect()
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

    /// An offsets layout reads back its texts with each of the four offset
    /// types, from a buffer that has other bytes before the first text.
    #[test]
    fn offsets_read_back_their_texts(texts in texts(), lead in 0usize..5) {
        // The first offset is `lead`, as in a slice of a longer column, so a
        // reader that assumes it is zero shows.
        let mut bytes = vec![b'x'; lead];
        let mut ends = vec![lead];
        for text in &texts {
            bytes.extend_from_slice(text.as_bytes());
            ends.push(bytes.len());
        }
        // SAFETY: every range between two ends is one of the texts, which are
        // UTF-8.
        let utf8 = unsafe { Utf8::new(&bytes) };
        let (i32s, i64s) = (offsets_of::<i32>(&ends), offsets_of::<i64>(&ends));
        let (u32s, u64s) = (offsets_of::<u32>(&ends), offsets_of::<u64>(&ends));
        // SAFETY: the offsets are the ends of the texts, in order, and the
        // last is the length of the bytes.
        let (a, b, c, d) = unsafe {
            (
                Offsets::new(&i32s, utf8),
                Offsets::new(&i64s, utf8),
                Offsets::new(&u32s, utf8),
                Offsets::new(&u64s, utf8),
            )
        };
        assert_texts(a, &texts)?;
        assert_texts(b, &texts)?;
        assert_texts(c, &texts)?;
        assert_texts(d, &texts)?;
        // The column takes one of the four layouts, which is enough to check
        // that `Column::text` counts the rows of an offsets layout.
        let column = Column::text(Text::Offsets(AnyOffsets::U64(d)), Marking::Never);
        prop_assert_eq!(column.map(|c| c.rows()), Ok(texts.len()));
    }

    /// A column of views reads back its texts, the short ones from the view
    /// and the long ones from one of two buffers.
    #[test]
    fn views_read_back_their_texts(texts in texts()) {
        // A long text goes to the buffer of its row's parity, so the buffer
        // number of a view is read and not assumed to be 0.
        let mut buffers = [Vec::new(), Vec::new()];
        let mut views = Vec::new();
        for (row, text) in texts.iter().enumerate() {
            let view = if text.len() <= INLINE {
                View::inline(text)
            } else {
                let buffer = &mut buffers[row % 2];
                let offset = u32::try_from(buffer.len()).expect("the buffer is small");
                buffer.extend_from_slice(text.as_bytes());
                View::outside(text, u32::try_from(row % 2).expect("0 or 1"), offset)
            };
            views.push(view.expect("the text fits a view"));
        }
        let lent = [&buffers[0][..], &buffers[1][..]];
        // SAFETY: each long view names the buffer and the offset that its text
        // was copied to, and the texts are UTF-8.
        let layout = unsafe { Views::new(&views, &lent) };
        assert_texts(layout, &texts)?;
        let column = Column::text(Text::Views(layout), Marking::Never);
        prop_assert_eq!(column.map(|c| c.rows()), Ok(texts.len()));
    }

    /// A run of pieces reads back its texts, contiguous and strided.
    #[test]
    fn pieces_read_back_their_texts(texts in texts(), step in 1usize..4) {
        // Each text's piece is followed by `step - 1` pieces of other texts, as
        // a field of a struct is followed by the struct's other fields.
        let pieces = texts
            .iter()
            .flat_map(|t| std::iter::once(Piece::new(t)).chain(std::iter::repeat_n(Piece::new("x"), step - 1)))
            .collect::<Vec<_>>();
        // SAFETY: the pointer is the vector's, the stride is `step` pieces in
        // bytes, and the count is the number of texts, each of which has
        // `step` pieces in the vector.
        let run = unsafe { Strided::from_raw(pieces.as_ptr(), step * size_of::<Piece<'_>>(), texts.len()) };
        assert_texts(run, &texts)?;
        let column = Column::text(Text::Pieces(run), Marking::Never);
        prop_assert_eq!(column.map(|c| c.rows()), Ok(texts.len()));
    }

    /// numpy `S` cells read back their texts, for a cell as wide as the
    /// longest text and for a wider one, contiguous and stepped, and
    /// `Fixed::new` refuses one row too many and a step below the width.
    #[test]
    fn byte_cells_read_back_their_texts(texts in texts(), slack in 0usize..3, step in 1usize..3) {
        // A cell has the bytes of the longest text plus `slack`, so the
        // longest text fills its cell with no zero after it when `slack` is 0.
        let width = texts.iter().map(String::len).max().unwrap_or(0) + slack;
        let mut bytes = Vec::new();
        for text in &texts {
            let start = bytes.len();
            bytes.extend_from_slice(text.as_bytes());
            bytes.resize(start + width, 0);
            // `step - 1` cells of other bytes lie between two rows, as in a
            // stepped numpy view.
            bytes.resize(start + step * width, b'y');
        }
        // SAFETY: every cell that the layout names holds a text, which is
        // UTF-8, padded with zeros.
        let units = unsafe { Units::utf8(&bytes) };
        let layout = Fixed::new(width, step * width, texts.len(), units).expect("the cells hold every row");
        assert_texts(layout, &texts)?;
        // One more row would reach past the bytes, and a step below the width
        // would overlap the cells, so both are refused when a cell has bytes.
        if width > 0 {
            prop_assert!(Fixed::new(width, step * width, texts.len() + 1, units).is_none());
            prop_assert!(Fixed::new(width, width - 1, texts.len(), units).is_none());
        }
        let column = Column::text(Text::Bytes(layout), Marking::Never);
        prop_assert_eq!(column.map(|c| c.rows()), Ok(texts.len()));
    }

    /// numpy `U` cells read back the characters of their texts as units.
    #[test]
    fn utf32_cells_read_back_their_texts(texts in texts(), slack in 0usize..3) {
        // A text is compared as the `u32` values of its characters, because
        // the reader returns units and no `char`.
        let expected = texts.iter().map(|t| t.chars().map(u32::from).collect::<Vec<_>>()).collect::<Vec<_>>();
        let width = expected.iter().map(Vec::len).max().unwrap_or(0) + slack;
        let mut units = Vec::new();
        for text in &expected {
            let start = units.len();
            units.extend_from_slice(text);
            units.resize(start + width, 0);
        }
        let layout = Fixed::new(width, width, texts.len(), Units::utf32(&units)).expect("the cells hold every row");
        prop_assert_eq!(layout.len(), expected.len());
        for (row, text) in expected.iter().enumerate() {
            prop_assert_eq!(layout.get(row), text.as_slice(), "row {}", row);
        }
        let column = Column::text(Text::Utf32(layout), Marking::Never);
        prop_assert_eq!(column.map(|c| c.rows()), Ok(texts.len()));
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
