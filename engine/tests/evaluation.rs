//! Tests of the executor. The first part runs rules over columns built by
//! hand and checks the outcome of each record. The second part generates
//! random rules and batches and checks that the executor agrees with a
//! simple reference interpreter, which evaluates one record at a time.

// The half-precision column holds nightly's `f16` (ADR-0036).
#![feature(f16)]
// The interpreter and the generators convert numbers with `as` on purpose,
// because the language converts numbers the way `as` does.
#![expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "numbers round as the language converts them"
)]

mod common;

use std::cmp::Ordering;

use proptest::collection::vec;
use proptest::prelude::*;
use proptest::sample::select;
use trapperkeeper_engine::Rule;
use trapperkeeper_engine::column::{
    AnyOffsets, Bits, Booleans, Column, Fixed, Floats, Ints, List, Marking, Offsets, PerRecord,
    Piece, Scalar, Shape, Strided, Text, Units, Utf8,
};
use trapperkeeper_engine::exec::{BindError, Fault, Outcome, SchemaMismatch};
use trapperkeeper_engine::expr::{Expr, Segment};
use trapperkeeper_engine::plan::{FieldTree, plan};
use uuid::Uuid;

use common::{
    BINARY, LentTexts, MAX_ROWS, NUMERIC, TextBuffers, TextLayout, column_text, observing, packed,
    text_layout,
};

/// Builds the expression for the field `x`.
fn x() -> Expr {
    Expr::Field(vec![Segment::Key("x".into())])
}

/// Runs the rule `e` over a batch in which `column` is the field `x`, and
/// returns the outcome of each record. `None` means that the source has no
/// column for `x`.
fn outcomes(e: Expr, column: Option<Column<'_>>) -> Vec<Outcome> {
    let program = plan(&[observing(Uuid::from_u128(1), e)]).expect("the rule plans");
    // A rule that reads no field, such as `True`, takes no column.
    let columns: Vec<Option<Column<'_>>> = match program.fields().paths().len() {
        0 => Vec::new(),
        _ => vec![column],
    };
    // Without a column there is no row count to take, so the batch gets
    // three records, which the tests without a column expect.
    let rows = column.map_or(3, |c| c.rows());
    let bound = program.bind(&columns).expect("the columns fit the program");
    let run = bound.run(rows, &columns).expect("the batch fits the bind");
    let rule = &run.rules()[0];
    (0..rows).map(|row| rule.outcome(row)).collect()
}

/// A column of `values` whose rows have a value where `bitmap` sets their bit.
fn ints<'a>(values: &'a [i32], bitmap: &'a [u8]) -> Column<'a> {
    let marking = Marking::Bitmap(Bits::new(bitmap, 0, values.len()).expect("one bit per row"));
    Column::ints(Ints::I32(Strided::from_slice(values)), marking).expect("one bit per value")
}

use Outcome::{Matched, NotEvaluated, NotMatched};

#[test]
fn a_comparison_matches_the_rows_whose_value_holds_it() {
    // The bitmap starts at bit 0 of its first byte.
    let bitmap = packed(&[true, true, false, true], 0);
    let column = ints(&[1, 7, 9, 9], &bitmap);
    // The third row has no value, so its comparison is unknown and the rule
    // does not match it.
    assert_eq!(
        outcomes(
            Expr::Gt(Box::new(x()), Box::new(Expr::Int(5))),
            Some(column)
        ),
        [NotMatched, Matched, NotMatched, Matched]
    );
}

#[test]
fn a_literal_on_the_left_compares_the_same_way() {
    let bitmap = packed(&[true; 3], 0);
    let column = ints(&[1, 5, 9], &bitmap);
    assert_eq!(
        outcomes(
            Expr::Lt(Box::new(Expr::Int(5)), Box::new(x())),
            Some(column)
        ),
        [NotMatched, NotMatched, Matched]
    );
}

#[test]
fn a_literal_outside_the_int_type_decides_every_row_with_a_value() {
    let values = [0u8, 200, 255];
    let column = Column::ints(Ints::U8(Strided::from_slice(&values)), Marking::Never)
        .expect("an unmarked column");
    assert_eq!(
        outcomes(
            Expr::Lt(Box::new(x()), Box::new(Expr::Int(300))),
            Some(column)
        ),
        [Matched; 3]
    );
    assert_eq!(
        outcomes(
            Expr::Eq(Box::new(x()), Box::new(Expr::Int(-1))),
            Some(column)
        ),
        [NotMatched; 3]
    );
}

#[test]
fn an_int_widens_to_a_float_literal() {
    let values = [2i64, 3];
    let column = Column::ints(Ints::I64(Strided::from_slice(&values)), Marking::Never)
        .expect("an unmarked column");
    assert_eq!(
        outcomes(
            Expr::Gt(Box::new(x()), Box::new(Expr::Float(2.5))),
            Some(column)
        ),
        [NotMatched, Matched]
    );
}

#[test]
fn a_float_literal_takes_the_type_of_the_column() {
    let values = [0.1f32, 0.2];
    let column = Column::floats(Floats::F32(Strided::from_slice(&values)), Marking::Never)
        .expect("an unmarked column");
    // The literal 0.1 rounds to the `f32` nearest to it, which is the value of
    // the first row.
    assert_eq!(
        outcomes(
            Expr::Eq(Box::new(x()), Box::new(Expr::Float(0.1))),
            Some(column)
        ),
        [Matched, NotMatched]
    );
}

#[test]
fn a_half_precision_column_compares_with_an_int_literal() {
    let values = [1.5f16, 2.0, -3.0];
    let column = Column::floats(Floats::F16(Strided::from_slice(&values)), Marking::Never)
        .expect("an unmarked column");
    assert_eq!(
        outcomes(
            Expr::Ge(Box::new(x()), Box::new(Expr::Int(2))),
            Some(column)
        ),
        [NotMatched, Matched, NotMatched]
    );
}

#[test]
fn a_nan_is_no_value() {
    let values = [f64::NAN, 1.0];
    let column = Column::floats(Floats::F64(Strided::from_slice(&values)), Marking::Never)
        .expect("an unmarked column");
    // `!=` holds for a NaN in IEEE 754, and the rule still does not match the
    // first row, because a NaN is a null and the comparison is unknown.
    assert_eq!(
        outcomes(
            Expr::Ne(Box::new(x()), Box::new(Expr::Int(5))),
            Some(column)
        ),
        [NotMatched, Matched]
    );
}

#[test]
fn a_field_that_the_source_lacks_matches_no_record() {
    // `outcomes` runs a batch of three records when it has no column.
    assert_eq!(
        outcomes(Expr::Lt(Box::new(x()), Box::new(Expr::Int(5))), None),
        [NotMatched; 3]
    );
}

#[test]
fn a_number_never_equals_a_text() {
    let pieces = [Piece::new("a"), Piece::new("b")];
    let bitmap = packed(&[true, false], 0);
    let marking = Marking::Bitmap(Bits::new(&bitmap, 0, 2).expect("one bit per row"));
    let column = Column::text(Text::Pieces(Strided::from_slice(&pieces)), marking)
        .expect("one bit per text");
    assert_eq!(
        outcomes(
            Expr::Eq(Box::new(x()), Box::new(Expr::Int(1))),
            Some(column)
        ),
        [NotMatched, NotMatched]
    );
    // The second row has no text, so `!=` is unknown there and does not
    // match. Under `==` a false row and an unknown row both give
    // `NotMatched`, so only `!=` shows that the second row is unknown.
    assert_eq!(
        outcomes(
            Expr::Ne(Box::new(x()), Box::new(Expr::Int(1))),
            Some(column)
        ),
        [Matched, NotMatched]
    );
}

#[test]
fn a_text_ordering_faults_where_the_text_is_there() {
    let pieces = [Piece::new("a"), Piece::new("b")];
    let bitmap = packed(&[true, false], 0);
    let marking = Marking::Bitmap(Bits::new(&bitmap, 0, 2).expect("one bit per row"));
    let column = Column::text(Text::Pieces(Strided::from_slice(&pieces)), marking)
        .expect("one bit per text");
    // The second row has no text, so the ordering is unknown there and
    // nothing faults.
    assert_eq!(
        outcomes(
            Expr::Lt(Box::new(x()), Box::new(Expr::Text("b".into()))),
            Some(column)
        ),
        [NotEvaluated(Fault::Unordered), NotMatched]
    );
}

#[test]
fn a_boolean_field_is_a_condition_and_compares_with_a_boolean() {
    // A byte that is not zero is true, and the third row has no value.
    let bytes = [0u8, 7, 1];
    let mask = [0u8, 0, 1];
    let column = Column::booleans(
        Booleans::Bytes(Strided::from_slice(&bytes)),
        Marking::ByteMask(Strided::from_slice(&mask)),
    )
    .expect("one byte of mask per row");
    assert_eq!(
        outcomes(x(), Some(column)),
        [NotMatched, Matched, NotMatched]
    );
    assert_eq!(
        outcomes(
            Expr::Eq(Box::new(x()), Box::new(Expr::Bool(false))),
            Some(column)
        ),
        [Matched, NotMatched, NotMatched]
    );
    // `<` on booleans is a fault, except on the row without a value, which
    // stays unknown.
    assert_eq!(
        outcomes(
            Expr::Lt(Box::new(x()), Box::new(Expr::Bool(true))),
            Some(column)
        ),
        [
            NotEvaluated(Fault::Unordered),
            NotEvaluated(Fault::Unordered),
            NotMatched
        ]
    );
}

#[test]
fn a_number_field_where_a_condition_is_expected_faults() {
    let values = [1i32];
    let column = Column::ints(Ints::I32(Strided::from_slice(&values)), Marking::Never)
        .expect("an unmarked column");
    assert_eq!(
        outcomes(x(), Some(column)),
        [NotEvaluated(Fault::NotBoolean)]
    );
}

#[test]
fn a_test_for_a_null_is_never_unknown() {
    let bitmap = packed(&[true, false], 0);
    let column = ints(&[1, 2], &bitmap);
    let none = || Box::new(Expr::None);
    assert_eq!(
        outcomes(Expr::IsNone(Box::new(x())), Some(column)),
        [NotMatched, Matched]
    );
    assert_eq!(
        outcomes(Expr::Ne(none(), Box::new(x())), Some(column)),
        [Matched, NotMatched]
    );
    // A field without a column is a null on every record.
    assert_eq!(
        outcomes(Expr::Eq(Box::new(x()), none()), None),
        [Matched; 3]
    );
}

#[test]
fn a_number_where_a_condition_is_expected_faults_on_every_record() {
    assert_eq!(
        outcomes(Expr::Int(1), None),
        [NotEvaluated(Fault::NotBoolean); 3]
    );
}

#[test]
fn a_false_left_operand_of_and_keeps_the_fault_on_its_right_from_happening() {
    let ys = [0i32, 1, 2];
    let pieces = [Piece::new("a"); 3];
    let y = Column::ints(Ints::I32(Strided::from_slice(&ys)), Marking::Never).expect("unmarked");
    let t =
        Column::text(Text::Pieces(Strided::from_slice(&pieces)), Marking::Never).expect("unmarked");
    let named = |name: &str| Box::new(Expr::Field(vec![Segment::Key(name.into())]));
    // `y != 0 and t > 1`, where `t` holds text, so its ordering faults on
    // every row.
    let rule = Expr::And(vec![
        Expr::Ne(named("y"), Box::new(Expr::Int(0))),
        Expr::Gt(named("t"), Box::new(Expr::Int(1))),
    ]);
    let program = plan(&[observing(Uuid::from_u128(1), rule)]).expect("the rule plans");
    // The planner numbers the fields in the order that the rule first reads
    // them, so `y` is field 0 and `t` field 1.
    let columns = [Some(y), Some(t)];
    let run = program
        .bind(&columns)
        .expect("one column per field")
        .run(3, &columns)
        .expect("the batch fits the bind");
    // The first row has `y == 0`, so the `and` is false there and the fault
    // of `t > 1` is ignored.
    let got: Vec<Outcome> = (0..3).map(|row| run.rules()[0].outcome(row)).collect();
    assert_eq!(
        got,
        [
            NotMatched,
            NotEvaluated(Fault::Unordered),
            NotEvaluated(Fault::Unordered)
        ]
    );
}

/// Runs the rule `e` over a batch with the columns `a` and `b`, and returns
/// the outcome of each record. The planner numbers fields in the order the
/// rule first reads them, so `e` must read `a` before `b`.
fn outcomes_of_pair(e: Expr, a: Column<'_>, b: Column<'_>) -> Vec<Outcome> {
    let program = plan(&[observing(Uuid::from_u128(1), e)]).expect("the rule plans");
    let columns = [Some(a), Some(b)];
    let run = program
        .bind(&columns)
        .expect("one column per field")
        .run(a.rows(), &columns)
        .expect("the batch fits the bind");
    (0..a.rows())
        .map(|row| run.rules()[0].outcome(row))
        .collect()
}

/// Builds the expression `a op b` for the fields `a` and `b`.
fn a_with_b(op: fn(Box<Expr>, Box<Expr>) -> Expr) -> Expr {
    let named = |name: &str| Box::new(Expr::Field(vec![Segment::Key(name.into())]));
    op(named("a"), named("b"))
}

#[test]
fn a_utf8_text_equals_its_utf32_cells() {
    let texts = [
        "caf\u{e9}".to_string(),
        "\u{1f600}".to_string(),
        "x".to_string(),
    ];
    let pieces: Vec<Piece<'_>> = texts.iter().map(|t| Piece::new(t)).collect();
    let utf8 =
        Column::text(Text::Pieces(Strided::from_slice(&pieces)), Marking::Never).expect("unmarked");
    // The cells hold the same texts as the pieces, except the last row, whose
    // `y` differs from `x`. Each row has four units, enough for `café`, and
    // the zero units after a shorter text end it.
    let mut cells = vec![0u32; 3 * 4];
    for (row, text) in ["caf\u{e9}", "\u{1f600}", "y"].iter().enumerate() {
        for (at, c) in text.chars().enumerate() {
            cells[4 * row + at] = u32::from(c);
        }
    }
    // The layout has three rows of four units, and each row starts four
    // units after the one before.
    let layout = Fixed::new(4, 4, 3, Units::utf32(&cells)).expect("the cells hold every row");
    let utf32 = Column::text(Text::Utf32(layout), Marking::Never).expect("unmarked");
    // `é` is two bytes in UTF-8 and one unit in UTF-32, and the emoji four
    // bytes and one unit, so only a comparison of code points finds them
    // equal.
    assert_eq!(
        outcomes_of_pair(a_with_b(Expr::Eq), utf8, utf32),
        [Matched, Matched, NotMatched]
    );
    // The swapped operands run the loop with the UTF-32 column on the left.
    assert_eq!(
        outcomes_of_pair(a_with_b(Expr::Ne), utf32, utf8),
        [NotMatched, NotMatched, Matched]
    );
}

/// Builds the expression `x in items`.
fn x_in(items: Vec<Expr>) -> Expr {
    Expr::In(Box::new(x()), Box::new(Expr::List(items)))
}

#[test]
fn a_membership_list_matches_in_any_order() {
    let values = [1i64, 2];
    let column =
        Column::ints(Ints::I64(Strided::from_slice(&values)), Marking::Never).expect("unmarked");
    let (a, one) = (Expr::Text("a".into()), Expr::Int(1));
    // A text never equals an int, so the text adds nothing in either place
    // and neither order faults.
    assert_eq!(
        outcomes(x_in(vec![a.clone(), one.clone()]), Some(column)),
        [Matched, NotMatched]
    );
    assert_eq!(
        outcomes(x_in(vec![one, a]), Some(column)),
        [Matched, NotMatched]
    );
}

#[test]
fn a_none_in_the_list_matches_a_null() {
    let bitmap = packed(&[true, false, true], 0);
    let column = ints(&[1, 2, 3], &bitmap);
    // The second row has no value, which the `None` of the list matches.
    assert_eq!(
        outcomes(x_in(vec![Expr::Int(1), Expr::None]), Some(column)),
        [Matched, Matched, NotMatched]
    );
    // Without the `None`, the row without a value is unknown, and so is its
    // `not in`.
    assert_eq!(
        outcomes(
            Expr::NotIn(Box::new(x()), Box::new(Expr::List(vec![Expr::Int(1)]))),
            Some(column)
        ),
        [NotMatched, NotMatched, Matched]
    );
}

/// A membership set of more than eight values, the engine's `SPLAT_LIMIT`,
/// looks each row up by binary search.
#[test]
fn a_long_list_finds_its_values_by_binary_search() {
    let values: Vec<i32> = (0..100).collect();
    let column =
        Column::ints(Ints::I32(Strided::from_slice(&values)), Marking::Never).expect("unmarked");
    // The list holds the twelve even numbers from 10 to 32, in descending
    // order, which the membership step sorts at bind.
    let list = (5..17).rev().map(|i| Expr::Int(2 * i)).collect();
    let expected: Vec<Outcome> = (0..100)
        .map(|v| {
            if (10..=32).contains(&v) && v % 2 == 0 {
                Matched
            } else {
                NotMatched
            }
        })
        .collect();
    assert_eq!(outcomes(x_in(list), Some(column)), expected);
}

/// Builds the expression `needle in x`.
fn in_x(needle: Expr) -> Expr {
    Expr::In(Box::new(needle), Box::new(x()))
}

#[test]
fn a_text_literal_is_found_in_utf8_and_utf32_cells() {
    // `é` is two bytes in UTF-8 and one unit in UTF-32, so each layout finds
    // it in its own encoding. `cafe` holds a plain `e`, which neither
    // encoding takes for `é`.
    let texts = ["caf\u{e9}".to_string(), "cafe".to_string()];
    let rule = || in_x(Expr::Text("\u{e9}".into()));
    let pieces = TextBuffers::new(&texts, TextLayout::Pieces { step: 1 });
    // The cells have one unit of slack, so each text ends in a zero unit, and
    // they lie end to end.
    let cells = TextBuffers::new(&texts, TextLayout::Utf32 { slack: 1, step: 1 });
    for buffers in [pieces, cells] {
        let lent = buffers.lend();
        let column = Column::text(lent.text(), Marking::Never).expect("unmarked");
        assert_eq!(outcomes(rule(), Some(column)), [Matched, NotMatched]);
    }
}

#[test]
fn an_empty_needle_is_found_in_every_text() {
    let pieces = [Piece::new(""), Piece::new("abc"), Piece::new("x")];
    let bitmap = packed(&[true, true, false], 0);
    let marking = Marking::Bitmap(Bits::new(&bitmap, 0, 3).expect("one bit per row"));
    let column = Column::text(Text::Pieces(Strided::from_slice(&pieces)), marking)
        .expect("one bit per text");
    // The empty text holds the empty needle too, as `"" in ""` is true in
    // Python. The third row has no text, so its search is unknown.
    assert_eq!(
        outcomes(in_x(Expr::Text(String::new())), Some(column)),
        [Matched, Matched, NotMatched]
    );
}

#[test]
fn a_needle_of_another_type_matches_no_element() {
    // Three records: `[1, 2]`, `[]` and `[None]`.
    let values = [1i64, 2, 0];
    let mask = [0u8, 0, 1];
    let ends = [0u64, 2, 2, 3];
    let elements = Column::ints(
        Ints::I64(Strided::from_slice(&values)),
        Marking::ByteMask(Strided::from_slice(&mask)),
    )
    .expect("one mask byte per element");
    // SAFETY: the ends are in order, start at 0 and end at the number of
    // elements.
    let layout = unsafe { Offsets::new(&ends, &elements) };
    let column =
        Column::list(List::Offsets(AnyOffsets::U64(layout)), Marking::Never).expect("unmarked");
    let not_in = Expr::NotIn(Box::new(Expr::Text("a".into())), Box::new(x()));
    // A text never equals an int, so the search is false where every
    // element has a value, also for the empty list. The null element of the
    // third record could be anything, so the search is unknown there, and
    // so is its `not in`.
    assert_eq!(
        outcomes(not_in, Some(column)),
        [Matched, Matched, NotMatched]
    );
}

#[test]
fn a_number_in_a_text_faults() {
    let pieces = [Piece::new("5"), Piece::new("x")];
    let bitmap = packed(&[true, false], 0);
    let marking = Marking::Bitmap(Bits::new(&bitmap, 0, 2).expect("one bit per row"));
    let column = Column::text(Text::Pieces(Strided::from_slice(&pieces)), marking)
        .expect("one bit per text");
    // The second row has no text, so its search is unknown and does not
    // fault.
    assert_eq!(
        outcomes(in_x(Expr::Int(5)), Some(column)),
        [NotEvaluated(Fault::Incompatible), NotMatched]
    );
}

#[test]
fn a_negative_int_orders_below_every_u64() {
    let signed = [-1i8, 5, 5];
    let unsigned = [3u64, u64::MAX, 5];
    let i8s =
        Column::ints(Ints::I8(Strided::from_slice(&signed)), Marking::Never).expect("unmarked");
    let u64s =
        Column::ints(Ints::U64(Strided::from_slice(&unsigned)), Marking::Never).expect("unmarked");
    assert_eq!(
        outcomes_of_pair(a_with_b(Expr::Lt), i8s, u64s),
        [Matched, Matched, NotMatched]
    );
    // With the `u64` column on the left, the loop tests the right operand for
    // a negative value.
    assert_eq!(
        outcomes_of_pair(a_with_b(Expr::Ge), u64s, i8s),
        [Matched, Matched, Matched]
    );
}

#[test]
fn a_signed_and_an_unsigned_int_compare_exactly() {
    let signed = [-1i8, 127];
    let unsigned = [255u8, 127];
    let a = Column::ints(Ints::I8(Strided::from_slice(&signed)), Marking::Never).expect("unmarked");
    let b =
        Column::ints(Ints::U8(Strided::from_slice(&unsigned)), Marking::Never).expect("unmarked");
    // As `i8`, 255 would read as -1, and as `u8`, -1 would read as 255.
    assert_eq!(
        outcomes_of_pair(a_with_b(Expr::Eq), a, b),
        [NotMatched, Matched]
    );
}

#[test]
fn two_floats_compare_in_the_wider_type() {
    let halves = [0.1f16, 2.0];
    let doubles = [0.1f64, 2.0];
    let a = Column::floats(Floats::F16(Strided::from_slice(&halves)), Marking::Never)
        .expect("unmarked");
    let b = Column::floats(Floats::F64(Strided::from_slice(&doubles)), Marking::Never)
        .expect("unmarked");
    // The `f16` nearest to 0.1 widens to an `f64` that is not 0.1.
    assert_eq!(
        outcomes_of_pair(a_with_b(Expr::Eq), a, b),
        [NotMatched, Matched]
    );
}

#[test]
fn a_strided_column_over_three_chunks_compares_every_row() {
    // Every second value is a row, so the run has a stride of two values and
    // `pack`, the loop of the column module over a chunk, reads it one row at
    // a time. A chunk holds `CHUNK_ROWS` rows, 2048, so 5000 rows make three
    // chunks.
    let values: Vec<i64> = (0..10_000).collect();
    // SAFETY: the pointer is the vector's, the stride is two `i64`, and 5000
    // rows two values apart lie inside the 10000 values.
    let every_second = unsafe { Strided::from_raw(values.as_ptr(), 2 * size_of::<i64>(), 5000) };
    let column = Column::ints(Ints::I64(every_second), Marking::Never).expect("an unmarked column");
    let got = outcomes(
        Expr::Ge(Box::new(x()), Box::new(Expr::Int(6000))),
        Some(column),
    );
    // Row `r` holds `2 * r`, so the rows from 3000 on hold 6000 or more.
    let expected: Vec<Outcome> = (0..5000)
        .map(|r| if r >= 3000 { Matched } else { NotMatched })
        .collect();
    assert_eq!(got, expected);
}

#[test]
fn a_batch_of_another_type_is_refused() {
    let program = plan(&[observing(
        Uuid::from_u128(1),
        Expr::Lt(Box::new(x()), Box::new(Expr::Int(5))),
    )])
    .expect("the rule plans");
    let ints = [1i32];
    let bools = [1u8];
    let bound = program
        .bind(&[Some(
            Column::ints(Ints::I32(Strided::from_slice(&ints)), Marking::Never).expect("unmarked"),
        )])
        .expect("one column per field");
    let other = Column::booleans(Booleans::Bytes(Strided::from_slice(&bools)), Marking::Never)
        .expect("unmarked");
    assert_eq!(
        bound.run(1, &[Some(other)]).err(),
        Some(SchemaMismatch { field: 0 })
    );
}

// The property tests below compare the executor with a reference interpreter
// that evaluates a rule on one record at a time, as ADR-0005 and ADR-0041
// define it.

/// The width of a float field. A literal compared with the field rounds to
/// this width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Width {
    F16,
    F32,
    F64,
}

/// The value of one field of a record, as the reference interpreter reads it.
#[derive(Debug, Clone, PartialEq)]
enum Value {
    // Every int type fits in an `i128`, so the interpreter compares ints
    // exactly.
    Int(i128),
    Float(f64, Width),
    Bool(bool),
    Text(String),
    // The elements of a list, where `Null` is an element without a value.
    List(Vec<Value>),
    Null,
}

/// The result of an expression on one record: true, false, unknown or a
/// fault (ADR-0005).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tri {
    True,
    False,
    Unknown,
    Fault(Fault),
}

impl Tri {
    fn of(b: bool) -> Self {
        if b { Self::True } else { Self::False }
    }

    /// Returns the outcome of a rule whose expression gives this result.
    fn outcome(self) -> Outcome {
        match self {
            Self::True => Matched,
            Self::False | Self::Unknown => NotMatched,
            Self::Fault(f) => NotEvaluated(f),
        }
    }
}

/// Returns `i` for the field `f{i}` of a generated rule.
fn field_number(path: &[Segment]) -> usize {
    match path {
        [Segment::Key(name)] => name[1..].parse().expect("a generated field is named f<i>"),
        _ => unreachable!("a generated path is one key"),
    }
}

/// Evaluates the condition `e` on one record, where `record[i]` is the
/// value of field `f{i}`.
fn reference(e: &Expr, record: &[Value]) -> Tri {
    match e {
        Expr::Bool(b) => Tri::of(*b),
        // `None` used as a condition is unknown.
        Expr::None => Tri::Unknown,
        Expr::Int(_) | Expr::Float(_) | Expr::Text(_) => Tri::Fault(Fault::NotBoolean),
        Expr::Field(p) => match record[field_number(p)] {
            Value::Bool(b) => Tri::of(b),
            Value::Null => Tri::Unknown,
            _ => Tri::Fault(Fault::NotBoolean),
        },
        Expr::Not(x) => match reference(x, record) {
            Tri::True => Tri::False,
            Tri::False => Tri::True,
            other => other,
        },
        Expr::And(items) => items.iter().fold(Tri::True, |left, item| match left {
            // A false left operand decides the `and`, so a fault on the right
            // is ignored. A fault on the left always counts, because the left
            // operand comes first.
            Tri::False | Tri::Fault(_) => left,
            _ => match reference(item, record) {
                Tri::True => left,
                right => right,
            },
        }),
        Expr::Or(items) => items.iter().fold(Tri::False, |left, item| match left {
            // A true left operand decides the `or`, so a fault on the right is
            // ignored. A fault on the left always counts.
            Tri::True | Tri::Fault(_) => left,
            _ => match reference(item, record) {
                Tri::False => left,
                right => right,
            },
        }),
        // `x == None` is a null test, which is never unknown (ADR-0005), so
        // its arms come before the comparisons.
        Expr::IsNone(x) => Tri::of(is_null(x, record)),
        Expr::IsNotNone(x) => Tri::of(!is_null(x, record)),
        Expr::Eq(l, r) if **r == Expr::None => Tri::of(is_null(l, record)),
        Expr::Eq(l, r) if **l == Expr::None => Tri::of(is_null(r, record)),
        Expr::Ne(l, r) if **r == Expr::None => Tri::of(!is_null(l, record)),
        Expr::Ne(l, r) if **l == Expr::None => Tri::of(!is_null(r, record)),
        Expr::Eq(l, r) => compare(l, r, record, Ordering::is_eq, false),
        Expr::Ne(l, r) => compare(l, r, record, Ordering::is_ne, false),
        Expr::Lt(l, r) => compare(l, r, record, Ordering::is_lt, true),
        Expr::Le(l, r) => compare(l, r, record, Ordering::is_le, true),
        Expr::Gt(l, r) => compare(l, r, record, Ordering::is_gt, true),
        Expr::Ge(l, r) => compare(l, r, record, Ordering::is_ge, true),
        // `x in y` with a field `y` searches the value of `y`. These arms come
        // before the membership arms, which take a written list.
        Expr::In(needle, y) if matches!(**y, Expr::Field(_)) => search(needle, y, record),
        Expr::NotIn(needle, y) if matches!(**y, Expr::Field(_)) => {
            match search(needle, y, record) {
                Tri::True => Tri::False,
                Tri::False => Tri::True,
                other => other,
            }
        }
        // `x in [a, b]` is `x == a or x == b` (ADR-0005), so the arm builds
        // that `or` and evaluates it with the arms above.
        Expr::In(needle, list) => reference(&either_equal(needle, list), record),
        Expr::NotIn(needle, list) => {
            reference(&Expr::Not(Box::new(either_equal(needle, list))), record)
        }
        _ => unreachable!("the generated rules hold no other kind"),
    }
}

/// Evaluates `needle in y`, where `y` is a field, on `record`. A text is
/// searched for the needle as a substring, and a list for an element that
/// equals the needle (ADR-0005).
fn search(needle: &Expr, y: &Expr, record: &[Value]) -> Tri {
    // `None in y` looks for a null element (ADR-0005), and a text, a number
    // or a boolean cannot be searched for `None`.
    if *needle == Expr::None {
        return match &record[field_number_of(y)] {
            Value::Null => Tri::Unknown,
            Value::List(items) => Tri::of(items.contains(&Value::Null)),
            _ => Tri::Fault(Fault::Incompatible),
        };
    }
    let needle = match needle {
        Expr::Field(p) => record[field_number(p)].clone(),
        literal => value_of(literal),
    };
    match (&record[field_number_of(y)], &needle) {
        // A search of a null, or for a null field, is unknown.
        (Value::Null, _) | (_, Value::Null) => Tri::Unknown,
        (Value::Text(text), Value::Text(n)) => Tri::of(text.contains(n.as_str())),
        // A list search is `any` over the equalities of its elements: true
        // when one matches, unknown when none does and one is a null, and
        // false otherwise, also for an empty list.
        (Value::List(items), n) => {
            if items.iter().any(|item| equal(n, item)) {
                Tri::True
            } else if items.contains(&Value::Null) {
                Tri::Unknown
            } else {
                Tri::False
            }
        }
        // A text cannot be searched for a needle that is no text, and a number
        // or a boolean cannot be searched at all, as Python raises a type
        // error.
        _ => Tri::Fault(Fault::Incompatible),
    }
}

/// Returns the number of the field `y`.
fn field_number_of(y: &Expr) -> usize {
    match y {
        Expr::Field(p) => field_number(p),
        _ => unreachable!("a generated search has a field on its right"),
    }
}

/// Returns the value of a number, boolean or text literal. The width of a
/// float plays no part in `order_of_fields`, which compares floats as `f64`.
/// It is `F64` here, the width of the generated list elements.
fn value_of(literal: &Expr) -> Value {
    match literal {
        Expr::Int(i) => Value::Int((*i).into()),
        Expr::Float(x) => Value::Float(*x, Width::F64),
        Expr::Bool(b) => Value::Bool(*b),
        Expr::Text(t) => Value::Text(t.clone()),
        _ => unreachable!("a generated needle is a field or a literal"),
    }
}

/// Whether the needle `n` equals the element `item`. A null element equals
/// nothing.
fn equal(n: &Value, item: &Value) -> bool {
    let kind = |v: &Value| match v {
        Value::Int(_) | Value::Float(..) => Some(Kind::Number),
        Value::Bool(_) => Some(Kind::Bool),
        Value::Text(_) => Some(Kind::Text),
        Value::List(_) => Some(Kind::List),
        Value::Null => None,
    };
    // `order_of_fields` answers orderings and faults on two kinds, which an
    // equality turns into false. A list needle never gets here, because bind
    // refuses it.
    kind(n) == kind(item) && order_of_fields(n, item, false) == Ok(Some(Ordering::Equal))
}

/// Builds `needle == a or needle == b ...` for the items of the list `list`.
fn either_equal(needle: &Expr, list: &Expr) -> Expr {
    let Expr::List(items) = list else {
        unreachable!("a generated membership has a list on its right")
    };
    Expr::Or(
        items
            .iter()
            .map(|item| Expr::Eq(Box::new(needle.clone()), Box::new(item.clone())))
            .collect(),
    )
}

/// Whether `e`, a field or a literal, is a null on `record`.
fn is_null(e: &Expr, record: &[Value]) -> bool {
    match e {
        Expr::Field(p) => record[field_number(p)] == Value::Null,
        other => *other == Expr::None,
    }
}

/// Evaluates `l op r` on `record`. `holds` says whether `op` is true for a
/// given order of the two values, and `ordering` is true when `op` is `<`,
/// `<=`, `>` or `>=`.
fn compare(
    l: &Expr,
    r: &Expr,
    record: &[Value],
    holds: fn(Ordering) -> bool,
    ordering: bool,
) -> Tri {
    let (lk, rk) = (kind(l, record), kind(r, record));
    // Values of two kinds, such as an int and a text, are never equal, as in
    // Python, so `==` is false and `!=` true. A null has no kind and stays
    // unknown below.
    if !ordering && lk.zip(rk).is_some_and(|(a, b)| a != b) {
        return Tri::of(!holds(Ordering::Equal));
    }
    let order = match (l, r) {
        (Expr::Field(a), Expr::Field(b)) => {
            order_of_fields(&record[field_number(a)], &record[field_number(b)], ordering)
        }
        (Expr::Field(p), literal) => {
            order_with_literal(&record[field_number(p)], literal, ordering)
        }
        // `order_with_literal` takes the field first, so `5 < x` compares
        // `x` with 5 and reverses the order.
        (literal, Expr::Field(p)) => {
            order_with_literal(&record[field_number(p)], literal, ordering)
                .map(|o| o.map(Ordering::reverse))
        }
        (a, b) => order_of_literals(a, b, ordering).map(Some),
    };
    match order {
        Ok(Some(o)) => Tri::of(holds(o)),
        Ok(None) => Tri::Unknown,
        Err(fault) => Tri::Fault(fault),
    }
}

/// The kinds of value that can be equal to each other: an int equals a float,
/// and a boolean, a text or a list equals only its own kind. Bind refuses to
/// compare two lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Number,
    Bool,
    Text,
    List,
}

/// Returns the kind of `e`, a field or a literal, on `record`, or `None` when
/// it is a null.
fn kind(e: &Expr, record: &[Value]) -> Option<Kind> {
    match e {
        Expr::Field(p) => match record[field_number(p)] {
            Value::Int(_) | Value::Float(..) => Some(Kind::Number),
            Value::Bool(_) => Some(Kind::Bool),
            Value::Text(_) => Some(Kind::Text),
            Value::List(_) => Some(Kind::List),
            Value::Null => None,
        },
        Expr::Int(_) | Expr::Float(_) => Some(Kind::Number),
        Expr::Bool(_) => Some(Kind::Bool),
        Expr::Text(_) => Some(Kind::Text),
        _ => unreachable!("a generated operand is a field or a literal"),
    }
}

/// Compares the field value `v` with `literal`. Returns `None` when `v` is
/// a null, and a fault when the pair cannot be ordered. `ordering` is true
/// for `<`, `<=`, `>` and `>=`.
fn order_with_literal(
    v: &Value,
    literal: &Expr,
    ordering: bool,
) -> Result<Option<Ordering>, Fault> {
    let order = match (v, literal) {
        (Value::Null, _) => return Ok(None),
        (Value::Bool(a), Expr::Bool(b)) => booleans(*a, *b, ordering)?,
        (Value::Int(i), Expr::Int(j)) => i.cmp(&i128::from(*j)),
        // An int widens to a float.
        (Value::Int(i), Expr::Float(x)) => ieee(*i as f64, *x),
        // A literal takes the width of the float field, so it rounds to the
        // nearest value of that width.
        (Value::Float(f, w), Expr::Int(j)) => ieee(*f, int_as(*j, *w)),
        (Value::Float(f, w), Expr::Float(x)) => ieee(*f, float_as(*x, *w)),
        // Two texts are equal when their characters are, whatever layout
        // the column has. An ordering of texts falls through to the fault.
        (Value::Text(a), Expr::Text(b)) if !ordering => a.cmp(b),
        // `reference` turns every `== None` into a null test, so no `None`
        // gets here. `compare` decides the equality of two kinds, so only an
        // ordering with a value that is not a number gets here.
        _ => return Err(Fault::Unordered),
    };
    Ok(Some(order))
}

/// Compares the values of two fields. Returns `None` when either is a
/// null, and a fault when the pair cannot be ordered. `ordering` is true
/// for `<`, `<=`, `>` and `>=`.
fn order_of_fields(a: &Value, b: &Value, ordering: bool) -> Result<Option<Ordering>, Fault> {
    let order = match (a, b) {
        (Value::Null, _) | (_, Value::Null) => return Ok(None),
        (Value::Bool(x), Value::Bool(y)) => booleans(*x, *y, ordering)?,
        (Value::Int(i), Value::Int(j)) => i.cmp(j),
        // Two floats compare in the wider of their two widths, which holds both
        // values exactly. An int widens to `f64`.
        (Value::Float(x, _), Value::Float(y, _)) => ieee(*x, *y),
        (Value::Int(i), Value::Float(y, _)) => ieee(*i as f64, *y),
        (Value::Float(x, _), Value::Int(j)) => ieee(*x, *j as f64),
        // Two texts are equal when their characters are, whatever layouts
        // and encodings the two columns have.
        (Value::Text(x), Value::Text(y)) if !ordering => x.cmp(y),
        // `compare` decides the equality of two kinds, so only an ordering
        // with a value that is not a number gets here.
        _ => return Err(Fault::Unordered),
    };
    Ok(Some(order))
}

/// Compares two literals, converting an int to `f64` when needed, or
/// returns a fault when the pair cannot be ordered. A generated rule may
/// compare two literals, which the planner computes ahead of time.
/// `ordering` is true for `<`, `<=`, `>` and `>=`.
fn order_of_literals(a: &Expr, b: &Expr, ordering: bool) -> Result<Ordering, Fault> {
    match (a, b) {
        (Expr::Bool(x), Expr::Bool(y)) => booleans(*x, *y, ordering),
        (Expr::Text(x), Expr::Text(y)) if !ordering => Ok(x.cmp(y)),
        // `compare` decides `True == 1` and `"a" == 1`, so only an ordering
        // gets here.
        (Expr::Bool(_) | Expr::Text(_), _) | (_, Expr::Bool(_) | Expr::Text(_)) => {
            Err(Fault::Unordered)
        }
        (Expr::Int(i), Expr::Int(j)) => Ok(i.cmp(j)),
        _ => Ok(ieee(number(a), number(b))),
    }
}

/// Compares two booleans for `==` and `!=`. Returns a fault for `<`, `<=`,
/// `>` and `>=`, because only numbers can be ordered.
fn booleans(a: bool, b: bool, ordering: bool) -> Result<Ordering, Fault> {
    if ordering {
        Err(Fault::Unordered)
    } else {
        Ok(a.cmp(&b))
    }
}

/// Compares two floats that are not NaN the way IEEE 754 does, so `-0.0`
/// equals `0.0`.
fn ieee(a: f64, b: f64) -> Ordering {
    a.partial_cmp(&b).expect("a NaN is a null and no literal")
}

/// Converts a number literal to `f64`.
fn number(e: &Expr) -> f64 {
    match e {
        Expr::Int(i) => *i as f64,
        Expr::Float(x) => *x,
        _ => unreachable!("a generated literal is a number"),
    }
}

/// Rounds `x` to the nearest float of width `w`.
fn float_as(x: f64, w: Width) -> f64 {
    match w {
        Width::F16 => f64::from(x as f16),
        Width::F32 => f64::from(x as f32),
        Width::F64 => x,
    }
}

/// Rounds `i` to the nearest float of width `w` in a single rounding, as
/// the executor does. Converting to `f64` first and then to `f32` would
/// round twice, which can give a different value.
fn int_as(i: i64, w: Width) -> f64 {
    match w {
        Width::F16 => f64::from(i as f16),
        Width::F32 => f64::from(i as f32),
        Width::F64 => i as f64,
    }
}

/// The values of one generated field. A buffer of numbers or of byte
/// booleans holds `step` values per row, and row `r` is the value at index
/// `r * step`.
#[derive(Debug, Clone)]
enum Values {
    I8(Vec<i8>),
    I16(Vec<i16>),
    I32(Vec<i32>),
    I64(Vec<i64>),
    U8(Vec<u8>),
    U16(Vec<u16>),
    U32(Vec<u32>),
    U64(Vec<u64>),
    F16(Vec<f16>),
    F32(Vec<f32>),
    F64(Vec<f64>),
    // One byte per value, where a byte that is not zero is true.
    BoolBytes(Vec<u8>),
    // The booleans, and the bytes that hold them one bit per row.
    BoolBits(Vec<bool>, Vec<u8>),
    // The texts, and their buffers in one layout.
    Text(Vec<String>, TextLayout, TextBuffers),
    List(ListValues),
}

/// The values of a generated list field: the elements of all records end to
/// end, which elements have a value, and where each record ends.
#[derive(Debug, Clone)]
struct ListValues {
    elements: Elements,
    // One byte per element, 1 for an element without a value.
    mask: Vec<u8>,
    // `ends[i]` is the index of the first element of record `i`, and the last
    // entry is the number of elements. These are the offsets of an offsets
    // layout.
    offsets: Vec<u64>,
    // Whether each record is a column of its own, as in a numpy array of
    // dtype `object`, or a range of one element column.
    per_record: bool,
}

/// The elements of a generated list field, of one type.
#[derive(Debug, Clone)]
enum Elements {
    Ints(Vec<i64>),
    Floats(Vec<f64>),
    // One byte per element, where a byte that is not zero is true.
    Bools(Vec<u8>),
    // The texts, and their bytes end to end with the offsets of each text,
    // which an offsets layout of the elements reads.
    Texts(Vec<String>, Vec<u8>, Vec<u32>),
}

impl ListValues {
    /// Builds the values of records whose elements are `elements`, `present`
    /// says which elements have a value, and record `i` holds `sizes[i]`
    /// elements.
    fn new(elements: Elements, present: &[bool], sizes: &[usize], per_record: bool) -> Self {
        let offsets = std::iter::once(0)
            .chain(sizes.iter().scan(0, |end, n| {
                *end += u64::try_from(*n).expect("a short list");
                Some(*end)
            }))
            .collect();
        Self {
            elements,
            mask: present.iter().map(|&p| u8::from(!p)).collect(),
            offsets,
            per_record,
        }
    }

    /// Returns the range of elements of record `row`.
    fn range(&self, row: usize) -> std::ops::Range<usize> {
        let at = |i: usize| usize::try_from(self.offsets[i]).expect("an end inside the elements");
        at(row)..at(row + 1)
    }

    /// Builds the column of the elements in `range`.
    fn elements(&self, range: std::ops::Range<usize>) -> Column<'_> {
        let marking = Marking::ByteMask(Strided::from_slice(&self.mask[range.clone()]));
        let column = match &self.elements {
            Elements::Ints(v) => Column::ints(Ints::I64(Strided::from_slice(&v[range])), marking),
            Elements::Floats(v) => {
                Column::floats(Floats::F64(Strided::from_slice(&v[range])), marking)
            }
            Elements::Bools(v) => {
                Column::booleans(Booleans::Bytes(Strided::from_slice(&v[range])), marking)
            }
            Elements::Texts(_, bytes, ends) => {
                // SAFETY: the bytes are the texts end to end, which are UTF-8,
                // and the ends of the texts in the range are in order inside
                // the bytes.
                let texts =
                    unsafe { Offsets::new(&ends[range.start..=range.end], Utf8::new(bytes)) };
                Column::text(Text::Offsets(AnyOffsets::U32(texts)), marking)
            }
        };
        column.expect("one mask byte per element")
    }

    /// Returns the shape of an element.
    fn scalar(&self) -> Scalar {
        match self.elements {
            Elements::Ints(_) => Scalar::Int,
            Elements::Floats(_) => Scalar::Float,
            Elements::Bools(_) => Scalar::Bool,
            Elements::Texts(..) => Scalar::Text,
        }
    }

    /// Returns the elements of record `row` for the reference interpreter.
    fn record(&self, row: usize) -> Vec<Value> {
        self.range(row)
            .map(|i| match &self.elements {
                // The mask arm comes first, because an element without a value
                // is a null whatever it holds.
                _ if self.mask[i] != 0 => Value::Null,
                Elements::Ints(v) => Value::Int(v[i].into()),
                // A NaN element is a null, as a NaN row is.
                Elements::Floats(v) if v[i].is_nan() => Value::Null,
                Elements::Floats(v) => Value::Float(v[i], Width::F64),
                Elements::Bools(v) => Value::Bool(v[i] != 0),
                Elements::Texts(texts, ..) => Value::Text(texts[i].clone()),
            })
            .collect()
    }

    /// Builds the values of record `row` alone.
    fn alone(&self, row: usize) -> Self {
        let range = self.range(row);
        let elements = match &self.elements {
            Elements::Ints(v) => Elements::Ints(v[range.clone()].to_vec()),
            Elements::Floats(v) => Elements::Floats(v[range.clone()].to_vec()),
            Elements::Bools(v) => Elements::Bools(v[range.clone()].to_vec()),
            Elements::Texts(texts, ..) => texts_elements(texts[range.clone()].to_vec()),
        };
        let present: Vec<bool> = self.mask[range.clone()].iter().map(|&m| m == 0).collect();
        Self::new(elements, &present, &[range.len()], self.per_record)
    }
}

/// Builds the text elements of `texts`, with their bytes end to end.
fn texts_elements(texts: Vec<String>) -> Elements {
    let bytes: Vec<u8> = texts.concat().into_bytes();
    let ends = std::iter::once(0)
        .chain(texts.iter().scan(0, |end, t| {
            *end += u32::try_from(t.len()).expect("a short text");
            Some(*end)
        }))
        .collect();
    Elements::Texts(texts, bytes, ends)
}

#[test]
fn none_in_a_list_finds_a_null_element() {
    // The records are `[1, None]`, `[2]` and `[]`.
    let elements = Elements::Ints(vec![1, 0, 2]);
    let values = ListValues::new(elements, &[true, false, true], &[2, 1, 0], false);
    let column = values.elements(0..3);
    // SAFETY: the ends are in order, start at 0 and end at the number of
    // elements.
    let layout = unsafe { Offsets::new(&values.offsets, &column) };
    let list =
        Column::list(List::Offsets(AnyOffsets::U64(layout)), Marking::Never).expect("unmarked");
    // `None in y` is true where an element is a null and false elsewhere,
    // also for the empty list, so it is never unknown on a list.
    assert_eq!(
        outcomes(in_x(Expr::None), Some(list)),
        [Matched, NotMatched, NotMatched]
    );
    assert_eq!(
        outcomes(Expr::NotIn(Box::new(Expr::None), Box::new(x())), Some(list)),
        [NotMatched, Matched, Matched]
    );
}

#[test]
fn the_same_rule_searches_a_text_on_one_schema_and_a_list_on_another() {
    // One program runs over two schemas, where `y` holds a text in the first
    // and a list of texts in the second.
    //
    // 1. It plans `x in y` once.
    // 2. It binds the program to a text column `y`, where `in` searches for a
    //    substring, so `"a"` is found in `"cat"`.
    // 3. It binds the same program to a list column `y`, where `in` searches
    //    for an element, so `"a"` is found only where an element is `"a"`.

    // 1. plan once
    let named = |name: &str| Box::new(Expr::Field(vec![Segment::Key(name.into())]));
    let rule = Expr::In(named("x"), named("y"));
    let program = plan(&[observing(Uuid::from_u128(1), rule)]).expect("the rule plans");
    let needles = [Piece::new("a"), Piece::new("a")];
    let x = Column::text(Text::Pieces(Strided::from_slice(&needles)), Marking::Never)
        .expect("unmarked");
    let run = |y: Column<'_>| {
        let columns = [Some(x), Some(y)];
        let outcomes = program
            .bind(&columns)
            .expect("one column per field")
            .run(2, &columns)
            .expect("the batch fits the bind");
        (0..2)
            .map(|row| outcomes.rules()[0].outcome(row))
            .collect::<Vec<_>>()
    };

    // 2. search a text
    let texts = [Piece::new("cat"), Piece::new("dog")];
    let y =
        Column::text(Text::Pieces(Strided::from_slice(&texts)), Marking::Never).expect("unmarked");
    assert_eq!(run(y), [Matched, NotMatched]);

    // 3. search a list
    // The records are `["cat", "a"]` and `["cat"]`.
    let elements = texts_elements(vec!["cat".into(), "a".into(), "cat".into()]);
    let values = ListValues::new(elements, &[true; 3], &[2, 1], false);
    let column = values.elements(0..3);
    // SAFETY: the ends are in order, start at 0 and end at the number of
    // elements.
    let layout = unsafe { Offsets::new(&values.offsets, &column) };
    let y = Column::list(List::Offsets(AnyOffsets::U64(layout)), Marking::Never).expect("unmarked");
    assert_eq!(run(y), [Matched, NotMatched]);
}

/// The borrowed buffers of a field whose column borrows parts that the field
/// does not hold itself.
enum Lent<'a> {
    Texts(LentTexts<'a>),
    // The element column of an offsets layout, or one column per record.
    Elements(Vec<Column<'a>>),
}

/// How a generated field marks its rows without a value.
#[derive(Debug, Clone, Copy)]
enum Mark {
    Never,
    // The bitmap starts at this bit of its first byte.
    Bitmap(usize),
    ByteMask,
}

/// One generated field: its values, the stride of its buffer, which rows
/// have a value, and the bitmap and the byte mask that say so.
#[derive(Debug, Clone)]
struct Field {
    values: Values,
    step: usize,
    mark: Mark,
    present: Vec<bool>,
    // The rows with a value when `mark` is `Bitmap(offset)`, from bit `offset`
    // of the first byte on, and empty otherwise.
    bitmap: Vec<u8>,
    // One byte per row, 1 for a row without a value. Every field holds it,
    // and only a field whose `mark` is `ByteMask` hands it over.
    mask: Vec<u8>,
}

/// A generated batch: its row count and its fields, where `None` is a field
/// with no column. The rules name field `i` as `f{i}`.
#[derive(Debug, Clone)]
struct Table {
    rows: usize,
    fields: Vec<Option<Field>>,
}

/// Describes every `step`-th value of `buffer`, `rows` values in all.
fn strided<T: Copy>(buffer: &[T], step: usize, rows: usize) -> Strided<'_, T> {
    assert!(
        rows == 0 || (rows - 1) * step < buffer.len(),
        "the rows lie inside the buffer"
    );
    // SAFETY: the pointer is the slice's, the stride is `step` values in
    // bytes, and the assertion keeps the last of the `rows` values inside
    // the slice.
    unsafe { Strided::from_raw(buffer.as_ptr(), step * size_of::<T>(), rows) }
}

impl Field {
    /// Borrows the buffers of a text or list field for its column. Returns
    /// `None` for a field of any other type. A text column borrows the pieces
    /// and the list of buffers that `LentTexts` holds, and a list column
    /// borrows its element columns, so they must live longer than the column,
    /// and `column` cannot make them itself.
    fn lend(&self) -> Option<Lent<'_>> {
        match &self.values {
            Values::Text(_, _, buffers) => Some(Lent::Texts(buffers.lend())),
            Values::List(l) if l.per_record => Some(Lent::Elements(
                (0..self.present.len())
                    .map(|r| l.elements(l.range(r)))
                    .collect(),
            )),
            // The mask has one byte per element, so its length is the number
            // of elements.
            Values::List(l) => Some(Lent::Elements(vec![l.elements(0..l.mask.len())])),
            _ => None,
        }
    }

    /// Builds the column that describes this field's buffers. `lent` holds
    /// what `lend` borrowed for a text or list field.
    fn column<'a>(&'a self, lent: Option<&'a Lent<'a>>) -> Column<'a> {
        let rows = self.present.len();
        let marking = match self.mark {
            Mark::Never => Marking::Never,
            Mark::Bitmap(offset) => {
                Marking::Bitmap(Bits::new(&self.bitmap, offset, rows).expect("one bit per row"))
            }
            Mark::ByteMask => Marking::ByteMask(Strided::from_slice(&self.mask)),
        };
        let s = self.step;
        let column = match &self.values {
            Values::I8(v) => Column::ints(Ints::I8(strided(v, s, rows)), marking),
            Values::I16(v) => Column::ints(Ints::I16(strided(v, s, rows)), marking),
            Values::I32(v) => Column::ints(Ints::I32(strided(v, s, rows)), marking),
            Values::I64(v) => Column::ints(Ints::I64(strided(v, s, rows)), marking),
            Values::U8(v) => Column::ints(Ints::U8(strided(v, s, rows)), marking),
            Values::U16(v) => Column::ints(Ints::U16(strided(v, s, rows)), marking),
            Values::U32(v) => Column::ints(Ints::U32(strided(v, s, rows)), marking),
            Values::U64(v) => Column::ints(Ints::U64(strided(v, s, rows)), marking),
            Values::F16(v) => Column::floats(Floats::F16(strided(v, s, rows)), marking),
            Values::F32(v) => Column::floats(Floats::F32(strided(v, s, rows)), marking),
            Values::F64(v) => Column::floats(Floats::F64(strided(v, s, rows)), marking),
            Values::BoolBytes(v) => Column::booleans(Booleans::Bytes(strided(v, s, rows)), marking),
            Values::BoolBits(_, bytes) => Column::booleans(
                Booleans::Bits(Bits::new(bytes, 0, rows).expect("one bit per row")),
                marking,
            ),
            Values::Text(..) => match lent {
                Some(Lent::Texts(t)) => Column::text(t.text(), marking),
                _ => unreachable!("a text field lends its texts"),
            },
            Values::List(l) => {
                let Some(Lent::Elements(columns)) = lent else {
                    unreachable!("a list field lends its elements")
                };
                let list = if l.per_record {
                    // `PerRecord` takes the shape of each record's column, a
                    // scalar, and the list is one level deeper.
                    let shape = Shape {
                        scalar: l.scalar(),
                        depth: 0,
                    };
                    List::PerRecord(PerRecord::new(columns, shape).expect("one shape"))
                } else {
                    // SAFETY: the ends are in order, start at 0 and end at the
                    // number of elements.
                    List::Offsets(AnyOffsets::U64(unsafe {
                        Offsets::new(&l.offsets, &columns[0])
                    }))
                };
                Column::list(list, marking)
            }
        };
        column.expect("the marking covers the rows")
    }

    /// Returns the value of row `row` for the reference interpreter.
    fn value(&self, row: usize) -> Value {
        if !self.present[row] {
            return Value::Null;
        }
        let at = row * self.step;
        let float = |f: f64, w| {
            if f.is_nan() {
                Value::Null
            } else {
                Value::Float(f, w)
            }
        };
        match &self.values {
            Values::I8(v) => Value::Int(v[at].into()),
            Values::I16(v) => Value::Int(v[at].into()),
            Values::I32(v) => Value::Int(v[at].into()),
            Values::I64(v) => Value::Int(v[at].into()),
            Values::U8(v) => Value::Int(v[at].into()),
            Values::U16(v) => Value::Int(v[at].into()),
            Values::U32(v) => Value::Int(v[at].into()),
            Values::U64(v) => Value::Int(v[at].into()),
            // A NaN is a null in every source.
            Values::F16(v) => float(v[at].into(), Width::F16),
            Values::F32(v) => float(v[at].into(), Width::F32),
            Values::F64(v) => float(v[at], Width::F64),
            Values::BoolBytes(v) => Value::Bool(v[at] != 0),
            // A bit-packed field has a step of 1, so its row is its index.
            Values::BoolBits(bools, _) => Value::Bool(bools[row]),
            Values::Text(texts, ..) => Value::Text(texts[row].clone()),
            Values::List(l) => Value::List(l.record(row)),
        }
    }
}

impl Field {
    /// Builds a copy of this field that holds only record `row`, with the same
    /// type, stride and marking.
    fn alone(&self, row: usize) -> Self {
        let at = row * self.step;
        // The one-row buffer keeps the stride, so it holds the value and the
        // filler values after it.
        let span = at..at + self.step;
        let values = match &self.values {
            Values::I8(v) => Values::I8(v[span].to_vec()),
            Values::I16(v) => Values::I16(v[span].to_vec()),
            Values::I32(v) => Values::I32(v[span].to_vec()),
            Values::I64(v) => Values::I64(v[span].to_vec()),
            Values::U8(v) => Values::U8(v[span].to_vec()),
            Values::U16(v) => Values::U16(v[span].to_vec()),
            Values::U32(v) => Values::U32(v[span].to_vec()),
            Values::U64(v) => Values::U64(v[span].to_vec()),
            Values::F16(v) => Values::F16(v[span].to_vec()),
            Values::F32(v) => Values::F32(v[span].to_vec()),
            Values::F64(v) => Values::F64(v[span].to_vec()),
            Values::BoolBytes(v) => Values::BoolBytes(v[span].to_vec()),
            // A bit-packed field has a step of 1, so its row is its index.
            Values::BoolBits(b, _) => Values::BoolBits(vec![b[row]], packed(&[b[row]], 0)),
            // The one-row buffers take their cell width from this text alone.
            // Bind only knows that the column holds texts, so the bound
            // program still runs over them.
            Values::Text(texts, layout, _) => {
                let one = [texts[row].clone()];
                Values::Text(one.to_vec(), *layout, TextBuffers::new(&one, *layout))
            }
            Values::List(l) => Values::List(l.alone(row)),
        };
        let present = vec![self.present[row]];
        let bitmap = match self.mark {
            Mark::Bitmap(offset) => packed(&present, offset),
            _ => Vec::new(),
        };
        Self {
            values,
            step: self.step,
            mark: self.mark,
            mask: vec![u8::from(!present[0])],
            present,
            bitmap,
        }
    }
}

impl Table {
    /// Builds a batch that holds only record `row`.
    fn alone(&self, row: usize) -> Self {
        Self {
            rows: 1,
            fields: self
                .fields
                .iter()
                .map(|f| f.as_ref().map(|f| f.alone(row)))
                .collect(),
        }
    }

    /// Returns record `row` for the reference interpreter: index `i` holds
    /// the value of field `f{i}`.
    fn record(&self, row: usize) -> Vec<Value> {
        self.fields
            .iter()
            .map(|f| f.as_ref().map_or(Value::Null, |f| f.value(row)))
            .collect()
    }

    /// Borrows the buffers of the text and list fields, one entry per field, for
    /// `columns`. The caller keeps the result in a binding of its own while
    /// it uses the columns.
    fn lend(&self) -> Vec<Option<Lent<'_>>> {
        self.fields
            .iter()
            .map(|f| f.as_ref().and_then(Field::lend))
            .collect()
    }

    /// Returns the columns of the fields in `fields`, in that order. `lent`
    /// holds the borrowed buffers that `lend` gives.
    fn columns<'a>(
        &'a self,
        lent: &'a [Option<Lent<'a>>],
        fields: &FieldTree,
    ) -> Vec<Option<Column<'a>>> {
        fields
            .paths()
            .iter()
            .map(|p| {
                let i = field_number(p);
                self.fields[i].as_ref().map(|f| f.column(lent[i].as_ref()))
            })
            .collect()
    }
}

/// Generates `n` ints of type `T`: mostly near 0, where the literals are,
/// and some from the whole range of the type.
fn int_values<T>(n: usize) -> impl Strategy<Value = Vec<T>>
where
    T: Arbitrary + TryFrom<i64> + Default + Clone + std::fmt::Debug + 'static,
{
    // A negative value that an unsigned type cannot hold becomes 0.
    let near_zero = (-6i64..6).prop_map(|i| T::try_from(i).unwrap_or_default());
    vec(prop_oneof![3 => near_zero, 1 => any::<T>()], n)
}

/// Generates a float: one near the literals, a NaN, or any float.
fn float() -> impl Strategy<Value = f64> {
    prop_oneof![
        // 0.1 rounds in `f16` and `f32`, 2.5 and -3.0 are exact in every
        // width, 1e10 overflows `f16`, and a NaN is a null.
        2 => select(vec![0.0, -0.0, 0.1, 1.0, 2.5, -3.0, 1e10, f64::NAN, f64::INFINITY]),
        1 => (-6i32..6).prop_map(f64::from),
        1 => any::<f64>(),
    ]
}

/// Generates the values of `rows` rows of any column type that this version
/// reads. A buffer with a stride holds `step` values per row.
fn values(rows: usize, step: usize) -> impl Strategy<Value = Values> {
    let n = rows * step;
    prop_oneof![
        int_values(n).prop_map(Values::I8),
        int_values(n).prop_map(Values::I16),
        int_values(n).prop_map(Values::I32),
        int_values(n).prop_map(Values::I64),
        int_values(n).prop_map(Values::U8),
        int_values(n).prop_map(Values::U16),
        int_values(n).prop_map(Values::U32),
        int_values(n).prop_map(Values::U64),
        vec(float(), n).prop_map(|v| Values::F16(v.into_iter().map(|f| f as f16).collect())),
        vec(float(), n).prop_map(|v| Values::F32(v.into_iter().map(|f| f as f32).collect())),
        vec(float(), n).prop_map(Values::F64),
        vec(any::<u8>(), n).prop_map(Values::BoolBytes),
        vec(any::<bool>(), rows).prop_map(|b| {
            let bytes = packed(&b, 0);
            Values::BoolBits(b, bytes)
        }),
        (vec(text_value(), rows), text_layout()).prop_map(|(texts, layout)| {
            let buffers = TextBuffers::new(&texts, layout);
            Values::Text(texts, layout, buffers)
        }),
        list_values(rows),
    ]
}

/// Generates the lists of `rows` records: up to four elements each, of ints,
/// floats, booleans or texts, with nulls among the elements and empty lists.
fn list_values(rows: usize) -> impl Strategy<Value = Values> {
    (vec(0usize..5, rows), any::<bool>(), 0..4usize).prop_flat_map(|(sizes, per_record, kind)| {
        let n: usize = sizes.iter().sum();
        let elements = match kind {
            0 => vec(-6i64..6, n).prop_map(Elements::Ints).boxed(),
            1 => vec(float(), n).prop_map(Elements::Floats).boxed(),
            // A byte of 2 is true, as a byte of 1 is.
            2 => vec(0u8..3, n).prop_map(Elements::Bools).boxed(),
            _ => vec(text_value(), n).prop_map(texts_elements).boxed(),
        };
        (elements, vec(prop::bool::weighted(0.8), n)).prop_map(move |(e, present)| {
            Values::List(ListValues::new(e, &present, &sizes, per_record))
        })
    })
}

/// The text literals of the generated rules: an empty text, a text of one
/// byte, a text of four-byte characters and a text longer than the 12 bytes
/// that an Arrow string view holds inline.
const LITERAL_TEXTS: [&str; 4] = ["", "a", "\u{1f600}\u{1f600}", "longer than a view holds"];

/// Generates the text of a row: half of the time one of the literals, so
/// that `==` matches some rows, and otherwise any text of a column.
fn text_value() -> impl Strategy<Value = String> {
    prop_oneof![
        1 => select(LITERAL_TEXTS.to_vec()).prop_map(String::from),
        1 => column_text(),
    ]
}

/// Generates a field of `rows` rows with a random type, stride and marking.
fn field(rows: usize) -> impl Strategy<Value = Field> {
    let mark = prop_oneof![
        Just(Mark::Never),
        (0usize..8).prop_map(Mark::Bitmap),
        Just(Mark::ByteMask)
    ];
    // Four rows in five have a value, so most comparisons see values and
    // some see nulls.
    (1usize..3, mark, vec(prop::bool::weighted(0.8), rows)).prop_flat_map(
        move |(step, mark, present)| {
            values(rows, step).prop_map(move |values| {
                // A bit-packed boolean column has one bit per row and no
                // stride, a text layout makes its own stride, and a list
                // field has no stride of its own.
                let step = if matches!(
                    values,
                    Values::Text(..) | Values::BoolBits(..) | Values::List(_)
                ) {
                    1
                } else {
                    step
                };
                let present = match mark {
                    Mark::Never => vec![true; rows],
                    _ => present.clone(),
                };
                let bitmap = match mark {
                    Mark::Bitmap(offset) => packed(&present, offset),
                    _ => Vec::new(),
                };
                let byte_mask = present.iter().map(|&p| u8::from(!p)).collect();
                Field {
                    values,
                    step,
                    mark,
                    present,
                    bitmap,
                    mask: byte_mask,
                }
            })
        },
    )
}

/// Generates a batch of up to `MAX_ROWS` rows with one to four fields, some
/// of which have no column. A chunk holds `CHUNK_ROWS` rows, 2048, so a
/// batch of more than 4096 rows crosses two chunk boundaries.
fn table() -> impl Strategy<Value = Table> {
    // Half of the batches have fewer than 70 rows, so many cases run fast and
    // still cross the boundary of a word of 64 rows.
    prop_oneof![0usize..70, 0..MAX_ROWS].prop_flat_map(|rows| {
        let f = prop_oneof![6 => field(rows).prop_map(Some), 1 => Just(None)];
        vec(f, 1..5).prop_map(move |fields| Table { rows, fields })
    })
}

/// Generates a literal: a boolean, a text, or a number near the generated
/// values or at the edge of a type.
fn literal() -> impl Strategy<Value = Expr> {
    prop_oneof![
        1 => any::<bool>().prop_map(Expr::Bool),
        1 => select(LITERAL_TEXTS.to_vec()).prop_map(|t| Expr::Text(t.into())),
        3 => (-6i64..6).prop_map(Expr::Int),
        // -129 lies below `i8`, 255 and 256 are the top of `u8` and one past
        // it, 65504 is the largest `f16`, 65520 the first int that `f16`
        // rounds to infinity, and 2^53 the edge past which `f64` no longer
        // holds every int.
        1 => select(vec![i64::MIN, -129, 255, 256, 65504, 65520, 1 << 53, i64::MAX]).prop_map(Expr::Int),
        2 => select(vec![0.0, -0.0, 0.1, 2.5, -3.0, 1e10, 1e300]).prop_map(Expr::Float),
    ]
}

/// Builds the expression for the field `f{i}`.
fn field_named(i: usize) -> Expr {
    Expr::Field(vec![Segment::Key(format!("f{i}"))])
}

/// Generates a condition over `n` fields, built from the expressions this
/// version supports: comparisons, null tests, membership in a written list,
/// `not`, `and`, `or`, fields and literals.
fn condition(n: usize) -> impl Strategy<Value = Expr> {
    // `==` and `!=` from `BINARY`, and the four orderings from `NUMERIC`.
    let ops: Vec<_> = BINARY[..2].iter().chain(&NUMERIC[..4]).copied().collect();
    let comparison =
        (select(ops.clone()), 0..n, literal(), any::<bool>()).prop_map(|(op, f, lit, flip)| {
            let (f, lit) = (Box::new(field_named(f)), Box::new(lit));
            if flip { op(lit, f) } else { op(f, lit) }
        });
    let fields = (select(ops.clone()), 0..n, 0..n)
        .prop_map(|(op, a, b)| op(Box::new(field_named(a)), Box::new(field_named(b))));
    // Two literals, which the planner computes ahead of time.
    let folded =
        (select(ops), literal(), literal()).prop_map(|(op, a, b)| op(Box::new(a), Box::new(b)));
    // Used as a condition, `None` is unknown, a boolean is itself, and a
    // number is a fault on every record.
    let literal_condition = prop_oneof![Just(Expr::None), literal()];
    let field_condition = (0..n).prop_map(field_named);
    let tested = prop_oneof![
        3 => (0..n).prop_map(field_named),
        1 => Just(Expr::None),
        1 => literal(),
    ];
    let null_test = (tested, 0..6usize).prop_map(|(x, form)| {
        let (x, none) = (Box::new(x), Box::new(Expr::None));
        match form {
            0 => Expr::IsNone(x),
            1 => Expr::IsNotNone(x),
            2 => Expr::Eq(x, none),
            3 => Expr::Eq(none, x),
            4 => Expr::Ne(x, none),
            _ => Expr::Ne(none, x),
        }
    });
    // A membership tests a field, a literal or `None` against a list of two
    // to four items drawn from literals, `None` and fields.
    let item = prop_oneof![
        3 => literal(),
        1 => Just(Expr::None),
        2 => (0..n).prop_map(field_named),
    ];
    let needle = prop_oneof![
        4 => (0..n).prop_map(field_named),
        1 => literal(),
        1 => Just(Expr::None),
    ];
    let membership = (needle, vec(item, 2..5), any::<bool>()).prop_map(|(x, items, negated)| {
        let (x, list) = (Box::new(x), Box::new(Expr::List(items)));
        if negated {
            Expr::NotIn(x, list)
        } else {
            Expr::In(x, list)
        }
    });
    // A search of a field for a literal, for another field or for `None`.
    // `y` may be a field of any type, and a number or a boolean there gives
    // the fault `Incompatible`.
    let sought = prop_oneof![literal(), (0..n).prop_map(field_named), Just(Expr::None)];
    let search = (sought, 0..n, any::<bool>()).prop_map(|(x, y, negated)| {
        let (x, y) = (Box::new(x), Box::new(field_named(y)));
        if negated {
            Expr::NotIn(x, y)
        } else {
            Expr::In(x, y)
        }
    });
    let leaf = prop_oneof![
        3 => search,
        6 => comparison,
        3 => fields,
        1 => folded,
        1 => literal_condition,
        2 => field_condition,
        2 => null_test,
        3 => membership,
    ];
    // Up to three levels of `not`, `and` and `or` over the leaves.
    // At most 24 nodes in all. An `and` or an `or` holds two or three items.
    leaf.prop_recursive(3, 24, 4, |inner| {
        prop_oneof![
            inner.clone().prop_map(|x| Expr::Not(Box::new(x))),
            vec(inner.clone(), 2..4).prop_map(Expr::And),
            vec(inner, 2..4).prop_map(Expr::Or),
        ]
    })
}

/// Generates one to three rules over `n` fields.
fn rules_over(n: usize) -> impl Strategy<Value = Vec<Rule>> {
    vec(condition(n), 1..4).prop_map(|es| {
        es.into_iter()
            .enumerate()
            .map(|(i, e)| observing(Uuid::from_u128(i as u128), e))
            .collect()
    })
}

proptest! {
    // `failure_persistence: None` turns off proptest's regression file, so a
    // failing run writes no untracked file into the working tree.
    #![proptest_config(ProptestConfig {
        failure_persistence: None,
        .. ProptestConfig::default()
    })]

    /// The executor gives every record of a batch the outcome that the
    /// reference interpreter gives it.
    #[test]
    fn the_executor_agrees_with_the_reference(
        (t, rules) in table().prop_flat_map(|t| {
            let n = t.fields.len();
            (Just(t), rules_over(n))
        })
    ) {
        // The test runs the rules through the planner, bind and the executor,
        // then checks each record against the reference interpreter.
        //
        // 1. It runs the rules as a sensor would, so the property covers the
        //    planner and bind as well as the executor.
        // 2. It checks each record against the reference interpreter, which
        //    evaluates one record at a time.

        // 1. plan, bind and run the rules
        let program = plan(&rules).expect("the generated kinds plan");
        let lent = t.lend();
        let columns = t.columns(&lent, program.fields());
        let bound = match program.bind(&columns) {
            // Bind refuses a rule that compares two list fields or searches
            // for a list field, and the property skips that rule set.
            Err(BindError::Unsupported { .. }) => return Ok(()),
            other => other?,
        };
        let outcomes = bound.run(t.rows, &columns)?;

        // 2. check each record against the reference
        for row in 0..t.rows {
            let record = t.record(row);
            // `rules()` holds one `RuleOutcome` per rule, in the order of the
            // rule set, and each answers for every row of the batch.
            for (rule, outcome) in rules.iter().zip(outcomes.rules()) {
                let expected = reference(&rule.expression, &record).outcome();
                prop_assert_eq!(outcome.outcome(row), expected, "{:?} at row {}", rule.expression, row);
            }
        }
    }

    /// A batch gives each record the outcome that the record gets alone.
    #[test]
    fn a_batch_gives_each_record_its_outcome_alone(
        (t, rules) in table().prop_flat_map(|t| {
            let n = t.fields.len();
            (Just(t), rules_over(n))
        })
    ) {
        // The test runs the rules over the whole batch and over records
        // alone, then compares the outcomes of each record.
        //
        // 1. It runs the rules over the whole batch, whose outcome each record
        //    alone must match.
        // 2. It runs the same bound program over a batch of one record for
        //    each record it checks. One bind serves both, because a batch of
        //    one record keeps the type of each field and the kind of its
        //    layout.
        // 3. It checks that the record has the same outcome in both runs.

        // 1. run the batch
        let program = plan(&rules).expect("the generated kinds plan");
        let lent = t.lend();
        let columns = t.columns(&lent, program.fields());
        let bound = match program.bind(&columns) {
            // Bind refuses a rule that compares two list fields or searches
            // for a list field, and the property skips that rule set.
            Err(BindError::Unsupported { .. }) => return Ok(()),
            other => other?,
        };
        let whole = bound.run(t.rows, &columns)?;

        // 2. run each record alone
        // A word holds 64 rows and a chunk 2048. The records at the edges of
        // the words and of the chunks, and every 97th record, keep a batch of
        // `MAX_ROWS` rows from costing a run of one record per row.
        let edges = [0, 1, 63, 64, 2047, 2048, 2049, 4095, 4096, t.rows.saturating_sub(1)];
        // 97 is a prime, so the sampled records fall at every place within a
        // word.
        let rows = edges.into_iter().chain((0..t.rows).step_by(97)).filter(|&r| r < t.rows);
        for row in rows {
            let one = t.alone(row);
            let lent = one.lend();
            let alone = bound.run(1, &one.columns(&lent, program.fields()))?;

            // 3. compare the outcomes of the record
            for (w, a) in whole.rules().iter().zip(alone.rules()) {
                prop_assert_eq!(w.outcome(row), a.outcome(0), "record {}", row);
            }
        }
    }
}
