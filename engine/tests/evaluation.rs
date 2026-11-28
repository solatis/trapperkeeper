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
    Bits, Booleans, Column, Floats, Ints, Marking, Piece, Strided, Text,
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
    // match. Only this check tells that row apart from a false one.
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
        _ => unreachable!("the generated rules hold no other kind"),
    }
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
/// and a boolean or a text equals only its own kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Number,
    Bool,
    Text,
}

/// Returns the kind of `e`, a field or a literal, on `record`. `None` for a
/// null.
fn kind(e: &Expr, record: &[Value]) -> Option<Kind> {
    match e {
        Expr::Field(p) => match record[field_number(p)] {
            Value::Int(_) | Value::Float(..) => Some(Kind::Number),
            Value::Bool(_) => Some(Kind::Bool),
            Value::Text(_) => Some(Kind::Text),
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
        // `literal()` never makes `None`, so no generated rule has `None < x`,
        // which validation would refuse. `compare` decides the equality of
        // two kinds, so only an ordering with a value that is not a number
        // gets here.
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
        // An ordering with a value that is not a number. Two texts never get
        // here, because bind refuses to compare two text fields.
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
    /// Borrows the buffers of a text field for its column, and `None` for a
    /// field of any other type.
    fn lend(&self) -> Option<LentTexts<'_>> {
        match &self.values {
            Values::Text(_, _, buffers) => Some(buffers.lend()),
            _ => None,
        }
    }

    /// Builds the column that describes this field's buffers. `lent` holds
    /// the borrowed buffers of a text field, as `lend` gives them.
    fn column<'a>(&'a self, lent: Option<&'a LentTexts<'a>>) -> Column<'a> {
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
            Values::Text(..) => Column::text(lent.expect("a text field is lent").text(), marking),
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
            Values::Text(texts, layout, _) => {
                let one = [texts[row].clone()];
                Values::Text(one.to_vec(), *layout, TextBuffers::new(&one, *layout))
            }
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

    /// Borrows the buffers of the text fields, one entry per field, for
    /// `columns`.
    fn lend(&self) -> Vec<Option<LentTexts<'_>>> {
        self.fields
            .iter()
            .map(|f| f.as_ref().and_then(Field::lend))
            .collect()
    }

    /// Returns the columns of the fields in `fields`, in that order. `lent`
    /// holds the borrowed buffers that `lend` gives.
    fn columns<'a>(
        &'a self,
        lent: &'a [Option<LentTexts<'a>>],
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
    ]
}

/// The text literals of the generated rules: an empty text, a text of one
/// byte, a text of four-byte characters and a text longer than the 12 bytes
/// that a view holds itself.
const LITERAL_TEXTS: [&str; 4] = ["", "a", "\u{1f600}\u{1f600}", "longer than a view holds"];

/// Generates the text of a row: mostly one of the literals, so that `==`
/// matches some rows, and otherwise any text of a column.
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
                // A text column and a bit-packed boolean column have no
                // stride of their own here, because the text layouts make
                // their own.
                let step = if matches!(values, Values::Text(..) | Values::BoolBits(..)) {
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
/// version supports: comparisons, null tests, `not`, `and`, `or`, fields and
/// literals.
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
    let leaf = prop_oneof![
        6 => comparison,
        3 => fields,
        1 => folded,
        1 => literal_condition,
        2 => field_condition,
        2 => null_test,
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
            // A generated rule may compare two fields whose types this
            // version does not compare, and that rule set does not run.
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
        //    one record keeps the type and the layout of each field.
        // 3. It checks that the record has the same outcome in both runs.

        // 1. run the batch
        let program = plan(&rules).expect("the generated kinds plan");
        let lent = t.lend();
        let columns = t.columns(&lent, program.fields());
        let bound = match program.bind(&columns) {
            // A generated rule may compare two fields whose types this
            // version does not compare, and that rule set does not run.
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
