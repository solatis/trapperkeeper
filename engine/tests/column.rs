//! The properties of the column description. Each one builds a description
//! from generated data and checks that a reader or a loop gives that data
//! back. The properties keep proptest's default case count, so that
//! `PROPTEST_CASES` can lower it under Miri.

// The half-precision column holds nightly's `f16` (ADR-0036).
#![feature(f16)]

mod common;

use std::mem::size_of;
use std::ops::Range;

use common::{
    MAX_ROWS, OffsetType, TextBuffers, TextLayout, assert_bitmap, chunk_of, packed, texts,
};
use proptest::collection::vec;
use proptest::prelude::*;
use proptest::sample::Index;
use trapperkeeper_engine::column::{
    AnyOffsets, AnySized, Bits, CHUNK_ROWS, CHUNK_WORDS, Column, Fixed, Float, Floats, Ints, Kind,
    List, Lists, Marking, Mismatch, Offsets, PerRecord, Present, Scalar, Shape, SizedLists,
    Strided, Text, Texts, Utf8, clear_nan, present_all, present_from_bits, present_from_mask,
};

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

/// Converts `numbers`, such as the ends or the sizes of lists, to the offset
/// type `O`.
fn offsets_of<O: TryFrom<usize>>(numbers: &[usize]) -> Vec<O> {
    numbers
        .iter()
        .map(|n| {
            // `expect` needs a `Debug` error, which `O::Error` need not be,
            // so the error is dropped.
            O::try_from(*n)
                .map_err(|_| ())
                .expect("every number fits the offset type")
        })
        .collect()
}

/// Lists of ints for the layouts, from empty lists to lists of seven ints.
fn lists() -> impl Strategy<Value = Vec<Vec<i64>>> {
    vec(vec(any::<i64>(), 0..8), 0..30)
}

/// The elements of all lists end to end, and the end of each list with a 0 in
/// front, which are the offsets of the lists.
fn flatten<T: Clone>(lists: &[Vec<T>]) -> (Vec<T>, Vec<usize>) {
    let elements = lists.concat();
    // Each end is the number of elements of the lists up to it, so `n` lists
    // give `n + 1` offsets.
    let ends = std::iter::once(0)
        .chain(lists.iter().scan(0, |end, l| {
            *end += l.len();
            Some(*end)
        }))
        .collect();
    (elements, ends)
}

/// Checks that the range of each record of `layout`, read from the element
/// values `elements`, equals the list of `expected` at that record.
// The bound on `Record` takes the layouts whose records are ranges, so the
// column-per-record layout has checks of its own.
fn assert_lists<'a, L: Lists<'a, Record = Range<usize>>>(
    layout: L,
    elements: &[i64],
    expected: &[Vec<i64>],
) -> Result<(), TestCaseError> {
    prop_assert_eq!(layout.len(), expected.len());
    for (row, list) in expected.iter().enumerate() {
        prop_assert_eq!(
            &elements[layout.get(row)],
            list.as_slice(),
            "record {}",
            row
        );
    }
    Ok(())
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
        let buffers = TextBuffers::new(&texts, TextLayout::Offsets { lead, offset: OffsetType::U64 });
        let lent = buffers.lend();
        for layout in lent.offsets().expect("an offsets layout") {
            match layout {
                AnyOffsets::I32(o) => assert_texts(o, &texts)?,
                AnyOffsets::I64(o) => assert_texts(o, &texts)?,
                AnyOffsets::U32(o) => assert_texts(o, &texts)?,
                AnyOffsets::U64(o) => assert_texts(o, &texts)?,
            }
        }
        // The column takes one of the four layouts, which is enough to check
        // that `Column::text` counts the rows of an offsets layout.
        let column = Column::text(lent.text(), Marking::Never);
        prop_assert_eq!(column.map(|c| c.rows()), Ok(texts.len()));
    }

    /// A column of views reads back its texts, the short ones from the view
    /// and the long ones from one of two buffers.
    #[test]
    fn views_read_back_their_texts(texts in texts()) {
        let buffers = TextBuffers::new(&texts, TextLayout::Views);
        let lent = buffers.lend();
        let Text::Views(layout) = lent.text() else { unreachable!("a views layout") };
        assert_texts(layout, &texts)?;
        let column = Column::text(Text::Views(layout), Marking::Never);
        prop_assert_eq!(column.map(|c| c.rows()), Ok(texts.len()));
    }

    /// A run of pieces reads back its texts, contiguous and strided.
    #[test]
    fn pieces_read_back_their_texts(texts in texts(), step in 1usize..4) {
        let buffers = TextBuffers::new(&texts, TextLayout::Pieces { step });
        let lent = buffers.lend();
        let Text::Pieces(run) = lent.text() else { unreachable!("a pieces layout") };
        assert_texts(run, &texts)?;
        let column = Column::text(Text::Pieces(run), Marking::Never);
        prop_assert_eq!(column.map(|c| c.rows()), Ok(texts.len()));
    }

    /// numpy `S` cells read back their texts, for a cell as wide as the
    /// longest text and for a wider one, contiguous and stepped, and
    /// `Fixed::new` refuses one row too many and a step below the width.
    #[test]
    fn byte_cells_read_back_their_texts(texts in texts(), slack in 0usize..3, step in 1usize..4) {
        // With a `slack` of 0, the longest text fills its cell with no zero
        // after it.
        let buffers = TextBuffers::new(&texts, TextLayout::Bytes { slack, step });
        let lent = buffers.lend();
        let Text::Bytes(layout) = lent.text() else { unreachable!("a bytes layout") };
        assert_texts(layout, &texts)?;
        // One more row would reach past the bytes, and a step below the width
        // would overlap the cells, so both are refused when a cell has bytes.
        let (units, width) = buffers.byte_cells().expect("a bytes layout");
        if width > 0 {
            prop_assert!(Fixed::new(width, step * width, texts.len() + 1, units).is_none());
            prop_assert!(Fixed::new(width, width - 1, texts.len(), units).is_none());
        }
        let column = Column::text(Text::Bytes(layout), Marking::Never);
        prop_assert_eq!(column.map(|c| c.rows()), Ok(texts.len()));
    }

    /// numpy `U` cells read back the characters of their texts as units,
    /// contiguous and stepped.
    #[test]
    fn utf32_cells_read_back_their_texts(texts in texts(), slack in 0usize..3, step in 1usize..4) {
        // A text is compared as the `u32` values of its characters, because
        // the reader returns units and no `char`.
        let expected = texts.iter().map(|t| t.chars().map(u32::from).collect::<Vec<_>>()).collect::<Vec<_>>();
        let buffers = TextBuffers::new(&texts, TextLayout::Utf32 { slack, step });
        let lent = buffers.lend();
        let Text::Utf32(layout) = lent.text() else { unreachable!("a UTF-32 layout") };
        prop_assert_eq!(layout.len(), expected.len());
        for (row, text) in expected.iter().enumerate() {
            prop_assert_eq!(layout.get(row), text.as_slice(), "row {}", row);
        }
        let column = Column::text(Text::Utf32(layout), Marking::Never);
        prop_assert_eq!(column.map(|c| c.rows()), Ok(texts.len()));
    }

    /// An offsets layout gives each list the rows of its elements, with each
    /// of the four offset types.
    #[test]
    fn offset_lists_read_back_their_elements(lists in lists()) {
        let (values, ends) = flatten(&lists);
        let elements = Column::ints(Ints::I64(Strided::from_slice(&values)), Marking::Never)
            .expect("`Never` fits any column");
        let (i32s, i64s) = (offsets_of::<i32>(&ends), offsets_of::<i64>(&ends));
        let (u32s, u64s) = (offsets_of::<u32>(&ends), offsets_of::<u64>(&ends));
        // SAFETY: `flatten` gives the offsets in order, starting at 0 and
        // ending at the number of elements.
        let (a, b, c, d) = unsafe {
            (
                Offsets::new(&i32s, &elements),
                Offsets::new(&i64s, &elements),
                Offsets::new(&u32s, &elements),
                Offsets::new(&u64s, &elements),
            )
        };
        assert_lists(a, &values, &lists)?;
        assert_lists(b, &values, &lists)?;
        assert_lists(c, &values, &lists)?;
        assert_lists(d, &values, &lists)?;
        // The column takes one of the four layouts, which is enough to check
        // that `Column::list` counts the records of an offsets layout.
        let column = Column::list(List::Offsets(AnyOffsets::I64(b)), Marking::Never);
        prop_assert_eq!(column.map(|c| c.rows()), Ok(lists.len()));
    }

    /// A fixed-width layout gives record `i` the elements from `i * step` on,
    /// contiguous and stepped, and refuses an element column that is too
    /// short.
    #[test]
    fn fixed_lists_read_back_their_elements(
        size in 0usize..5,
        extra in 0usize..3,
        len in 0usize..50,
        short in 1usize..4,
    ) {
        // `size + extra` elements lie between two records, as in a stepped
        // view of a numpy array.
        let step = size + extra;
        // The number of elements up to the end of the last record, and 0 when
        // there is no record.
        let end = len.checked_sub(1).map_or(0, |last| last * step + size);
        let values = (0..i64::try_from(end).expect("a small count")).collect::<Vec<_>>();
        let elements = Column::ints(Ints::I64(Strided::from_slice(&values)), Marking::Never)
            .expect("`Never` fits any column");
        let layout = Fixed::new(size, step, len, &elements).expect("the elements hold every record");
        let expected = (0..len).map(|i| values[i * step..i * step + size].to_vec()).collect::<Vec<_>>();
        assert_lists(layout, &values, &expected)?;
        // An element column `short` rows shorter than the end of the last
        // record is refused. The case is skipped when that end is below
        // `short`.
        if let Some(cut) = end.checked_sub(short) {
            let fewer = Column::ints(Ints::I64(Strided::from_slice(&values[..cut])), Marking::Never)
                .expect("`Never` fits any column");
            prop_assert!(Fixed::new(size, step, len, &fewer).is_none());
        }
        let column = Column::list(List::Fixed(layout), Marking::Never);
        prop_assert_eq!(column.map(|c| c.rows()), Ok(len));
    }

    /// A column per record reads back a list of masked float32 arrays, some
    /// contiguous and some stepped, and `PerRecord::new` refuses a record of
    /// another shape.
    #[test]
    fn per_record_lists_read_back(
        // Each mask has as many booleans as the longest array, and a record
        // uses the first ones.
        arrays in vec((vec(any::<f32>(), 0..40), vec(any::<bool>(), 40), 1usize..3), 0..20),
        mixed_at in any::<Index>(),
    ) {
        // Each record is a float32 column over its own values, with its own
        // step and a byte mask from its booleans, so the records differ in
        // layout and marking as in a numpy object array.
        let stepped = arrays.iter().map(|(xs, _, step)| xs.iter().step_by(*step).copied().collect::<Vec<_>>()).collect::<Vec<_>>();
        let masks = arrays
            .iter()
            .zip(&stepped)
            // A mask has one byte per value that the stepped run reads.
            .map(|((_, mask, _), values)| mask[..values.len()].iter().map(|b| u8::from(*b)).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        let records = arrays
            .iter()
            .zip(&stepped)
            .zip(&masks)
            .map(|(((xs, _, step), values), mask)| {
                // SAFETY: the pointer is the vector's, the stride is `step`
                // values in bytes, and the count is the number of `step`-th
                // values that the vector holds.
                let run = unsafe { Strided::from_raw(xs.as_ptr(), step * size_of::<f32>(), values.len()) };
                Column::floats(Floats::F32(run), Marking::ByteMask(Strided::from_slice(mask)))
                    .expect("the mask has a byte per value")
            })
            .collect::<Vec<_>>();
        let shape = Shape { scalar: Scalar::Float, depth: 0 };
        let layout = PerRecord::new(&records, shape).expect("every record is a float column");
        prop_assert_eq!(layout.len(), records.len());
        for (row, (values, mask)) in stepped.iter().zip(&masks).enumerate() {
            let Kind::Floats(Floats::F32(run), Marking::ByteMask(marks)) = layout.get(row).kind() else {
                return Err(TestCaseError::fail(format!("record {row} is not a masked float32 column")));
            };
            // The bits are compared, because a NaN is not equal to itself.
            prop_assert!((0..values.len()).all(|i| run.get(i).to_bits() == values[i].to_bits() && marks.get(i) == mask[i]));
        }
        // The record at `mixed_at` becomes an int column, and the constructor
        // names its index. A list without records has no record to replace.
        if !records.is_empty() {
            let at = mixed_at.index(records.len());
            let mut mixed = records.clone();
            mixed[at] = Column::ints(Ints::I64(Strided::from_slice(&[])), Marking::Never).expect("`Never` fits any column");
            prop_assert_eq!(PerRecord::new(&mixed, shape).err(), Some(at));
        }
        let column = Column::list(List::PerRecord(layout), Marking::Never).expect("`Never` fits any column");
        prop_assert_eq!(column.rows(), records.len());
        prop_assert_eq!(column.shape(), Shape { scalar: Scalar::Float, depth: 1 });
    }

    /// An offset-and-size layout gives each list its elements when the lists
    /// lie in reverse order and when two records share one list.
    #[test]
    fn sized_lists_read_back_their_elements(lists in lists()) {
        let (values, ends) = flatten(&lists);
        let elements = Column::ints(Ints::I64(Strided::from_slice(&values)), Marking::Never)
            .expect("`Never` fits any column");
        // The records name the lists from the last to the first, so their
        // offsets fall, and the last list is named twice, so two records
        // overlap fully.
        let order = (0..lists.len()).rev().chain(lists.len().checked_sub(1)).collect::<Vec<_>>();
        // `flatten` puts a 0 before the ends, so the end at `i` is the start of
        // list `i`.
        let starts = order.iter().map(|i| ends[*i]).collect::<Vec<_>>();
        let sizes = order.iter().map(|i| lists[*i].len()).collect::<Vec<_>>();
        let (offsets, sizes) = (offsets_of::<i64>(&starts), offsets_of::<i64>(&sizes));
        // SAFETY: each offset and size is one of the lists, which lie inside
        // the elements, and there is one size per offset.
        let layout = unsafe { SizedLists::new(&offsets, &sizes, &elements) };
        let expected = order.iter().map(|i| lists[*i].clone()).collect::<Vec<_>>();
        assert_lists(layout, &values, &expected)?;
        let column = Column::list(List::Sized(AnySized::I64(layout)), Marking::Never);
        prop_assert_eq!(column.map(|c| c.rows()), Ok(expected.len()));
    }

    /// A list of lists of ints, a list of texts, and masked arrays of masked
    /// arrays of float32 read back through every level.
    #[test]
    fn nested_lists_read_back(
        outer in vec(lists(), 0..10),
        texts in vec(texts(), 0..10),
        // Each mask has as many booleans as the longest array, and an array
        // uses the first ones.
        arrays in vec(vec((vec(any::<f32>(), 0..20), vec(any::<bool>(), 20)), 0..5), 0..5),
    ) {
        // The test describes three columns whose lists hold lists, texts and
        // columns, and reads each back through every level.
        //
        // 1. Flattening the lists of lists twice gives the inner lists with
        //    the outer offsets, and then the ints with the inner offsets.
        // 2. The columns are built from the inside out, because each list
        //    borrows its element column.
        // 3. Each outer record reads as a range of inner lists, and each of
        //    those as a range of ints, which must equal the input. The inner
        //    column has the shape of a list of ints.
        // 4. A list may hold texts. Its element column is a text column of
        //    offsets, and each record reads back as its texts.
        // 5. Masked arrays of masked arrays are a column per record inside a
        //    column per record, as a numpy object array that holds masked
        //    arrays, and every value and mask reads back through both levels.

        // 1. flatten twice
        let (inner, outer_ends) = flatten(&outer);
        let (values, inner_ends) = flatten(&inner);

        // 2. build from the inside out
        let ints = Column::ints(Ints::I64(Strided::from_slice(&values)), Marking::Never)
            .expect("`Never` fits any column");
        let (inner_offsets, outer_offsets) = (offsets_of::<i64>(&inner_ends), offsets_of::<i64>(&outer_ends));
        // SAFETY: `flatten` gives the offsets in order, starting at 0 and
        // ending at the number of elements.
        let inner_layout = unsafe { Offsets::new(&inner_offsets, &ints) };
        let inner_column = Column::list(List::Offsets(AnyOffsets::I64(inner_layout)), Marking::Never)
            .expect("`Never` fits any column");
        // SAFETY: as for the inner offsets.
        let outer_layout = unsafe { Offsets::new(&outer_offsets, &inner_column) };

        // 3. read the lists of lists
        prop_assert_eq!(outer_layout.len(), outer.len());
        for (row, lists) in outer.iter().enumerate() {
            let got = outer_layout.get(row).map(|i| values[inner_layout.get(i)].to_vec()).collect::<Vec<_>>();
            prop_assert_eq!(&got, lists, "record {}", row);
        }
        prop_assert_eq!(inner_column.shape(), Shape { scalar: Scalar::Int, depth: 1 });

        // 4. read the lists of texts
        let (all, text_ends) = flatten(&texts);
        let (bytes, byte_ends) = flatten(&all.iter().map(|t| t.as_bytes().to_vec()).collect::<Vec<_>>());
        let (byte_offsets, text_offsets) = (offsets_of::<i64>(&byte_ends), offsets_of::<i64>(&text_ends));
        // SAFETY: the bytes are the texts end to end, and the offsets are
        // their ends, in order, starting at 0.
        let text_layout = unsafe { Offsets::new(&byte_offsets, Utf8::new(&bytes)) };
        let text_column = Column::text(Text::Offsets(AnyOffsets::I64(text_layout)), Marking::Never)
            .expect("`Never` fits any column");
        // SAFETY: `flatten` gives the offsets in order, starting at 0 and
        // ending at the number of texts.
        let lists_of_texts = unsafe { Offsets::new(&text_offsets, &text_column) };
        for (row, record) in texts.iter().enumerate() {
            // `text_column` holds `text_layout`, so this reads the texts that
            // the record's range points at.
            let got = lists_of_texts.get(row).map(|i| text_layout.get(i).to_vec()).collect::<Vec<_>>();
            let want = record.iter().map(|t| t.as_bytes().to_vec()).collect::<Vec<_>>();
            prop_assert_eq!(got, want, "record {}", row);
        }

        // 5. read the masked arrays of masked arrays
        let masks = arrays
            .iter()
            .map(|record| record.iter().map(|(xs, mask)| mask[..xs.len()].iter().map(|b| u8::from(*b)).collect::<Vec<_>>()).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        let leaves = arrays
            .iter()
            .zip(&masks)
            .map(|(record, record_masks)| {
                record
                    .iter()
                    .zip(record_masks)
                    .map(|((xs, _), mask)| {
                        Column::floats(Floats::F32(Strided::from_slice(xs)), Marking::ByteMask(Strided::from_slice(mask)))
                            .expect("the mask has a byte per value")
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let float = Shape { scalar: Scalar::Float, depth: 0 };
        let middles = leaves
            .iter()
            .map(|l| {
                let records = PerRecord::new(l, float).expect("every leaf is a float column");
                Column::list(List::PerRecord(records), Marking::Never).expect("`Never` fits any column")
            })
            .collect::<Vec<_>>();
        let top = PerRecord::new(&middles, Shape { scalar: Scalar::Float, depth: 1 })
            .expect("every middle is a list of float columns");
        for (row, record) in arrays.iter().enumerate() {
            let Kind::List(List::PerRecord(middle), _) = top.get(row).kind() else {
                return Err(TestCaseError::fail(format!("record {row} is not a column per record")));
            };
            for (i, (xs, mask)) in record.iter().enumerate() {
                let Kind::Floats(Floats::F32(run), Marking::ByteMask(marks)) = middle.get(i).kind() else {
                    return Err(TestCaseError::fail(format!("record {row}, {i} is not a masked float32 column")));
                };
                // The bits are compared, because a NaN is not equal to itself.
                prop_assert!((0..xs.len()).all(|k| run.get(k).to_bits() == xs[k].to_bits() && marks.get(k) == u8::from(mask[k])));
            }
        }
        let column = Column::list(List::PerRecord(top), Marking::Never).expect("`Never` fits any column");
        prop_assert_eq!(column.shape(), Shape { scalar: Scalar::Float, depth: 2 });
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
