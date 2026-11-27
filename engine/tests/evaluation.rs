//! The tests of the executor: rules planned, bound and run over columns built
//! by hand.

// The half-precision column holds nightly's `f16` (ADR-0036).
#![feature(f16)]

mod common;

use trapperkeeper_engine::column::{
    Bits, Booleans, Column, Floats, Ints, Marking, Piece, Strided, Text,
};
use trapperkeeper_engine::exec::{Fault, Outcome, SchemaMismatch};
use trapperkeeper_engine::expr::{Expr, Segment};
use trapperkeeper_engine::plan::plan;
use uuid::Uuid;

use common::{observing, packed};

/// The field `x` of a record.
fn x() -> Expr {
    Expr::Field(vec![Segment::Key("x".into())])
}

/// The outcome of the rule `e` for each record of a batch where `column` is
/// the field `x`, or no column for a source that lacks `x`.
fn outcomes(e: Expr, column: Option<Column<'_>>) -> Vec<Outcome> {
    let program = plan(&[observing(Uuid::from_u128(1), e)]).expect("the rule plans");
    let columns: Vec<Option<Column<'_>>> = match program.fields().paths().len() {
        0 => Vec::new(),
        _ => vec![column],
    };
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
    assert_eq!(
        outcomes(Expr::Lt(Box::new(x()), Box::new(Expr::Int(5))), None),
        [NotMatched; 3]
    );
}

#[test]
fn a_number_against_a_text_field_faults_where_the_text_is_there() {
    let pieces = [Piece::new("a"), Piece::new("b")];
    let bitmap = packed(&[true, false], 0);
    let marking = Marking::Bitmap(Bits::new(&bitmap, 0, 2).expect("one bit per row"));
    let column = Column::text(Text::Pieces(Strided::from_slice(&pieces)), marking)
        .expect("one bit per text");
    // The second row has no text, so the comparison is unknown there and
    // nothing faults.
    assert_eq!(
        outcomes(
            Expr::Eq(Box::new(x()), Box::new(Expr::Int(1))),
            Some(column)
        ),
        [NotEvaluated(Fault::Incompatible), NotMatched]
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
fn a_strided_column_over_three_chunks_compares_every_row() {
    // Every second value is a row, so the run has a stride of two values and
    // the loop reads it one row at a time.
    let values: Vec<i64> = (0..10_000).collect();
    // SAFETY: the pointer is the vector's, the stride is two `i64`, and 5000
    // rows two values apart lie inside the 10000 values.
    let run = unsafe { Strided::from_raw(values.as_ptr(), 2 * size_of::<i64>(), 5000) };
    let column = Column::ints(Ints::I64(run), Marking::Never).expect("an unmarked column");
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
        bound.run(1, &[Some(other)]).map(|_| ()),
        Err(SchemaMismatch { field: 0 })
    );
}
