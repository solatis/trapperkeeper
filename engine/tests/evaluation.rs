//! The tests of the executor: rules planned, bound and run over columns built
//! by hand, and a property that compares the executor with a reference
//! interpreter on generated rules and batches.

// The half-precision column holds nightly's `f16` (ADR-0036).
#![feature(f16)]
// The interpreter and the generators round numbers with `as` on purpose,
// because the language converts a number as `as` does.
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
    AnyOffsets, Bits, Booleans, Column, Floats, Ints, Marking, Offsets, Piece, Strided, Text, Utf8,
};
use trapperkeeper_engine::exec::{Fault, Outcome, SchemaMismatch};
use trapperkeeper_engine::expr::{Expr, Segment};
use trapperkeeper_engine::plan::{FieldTree, plan};
use uuid::Uuid;

use common::{BINARY, MAX_ROWS, NUMERIC, observing, packed};

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
fn a_false_left_operand_of_and_keeps_the_fault_on_its_right_from_happening() {
    let ys = [0i32, 1, 2];
    let pieces = [Piece::new("a"); 3];
    let y = Column::ints(Ints::I32(Strided::from_slice(&ys)), Marking::Never).expect("unmarked");
    let t =
        Column::text(Text::Pieces(Strided::from_slice(&pieces)), Marking::Never).expect("unmarked");
    let named = |name: &str| Box::new(Expr::Field(vec![Segment::Key(name.into())]));
    // `y != 0 and t == 1`, where `t` holds text, so its comparison faults on
    // every row.
    let rule = Expr::And(vec![
        Expr::Ne(named("y"), Box::new(Expr::Int(0))),
        Expr::Eq(named("t"), Box::new(Expr::Int(1))),
    ]);
    let program = plan(&[observing(Uuid::from_u128(1), rule)]).expect("the rule plans");
    let columns = [Some(y), Some(t)];
    let run = program
        .bind(&columns)
        .expect("one column per field")
        .run(3, &columns)
        .expect("the batch fits the bind");
    // The first row has `y == 0`, so the `and` is false there and the fault
    // of its right operand does not happen.
    let got: Vec<Outcome> = (0..3).map(|row| run.rules()[0].outcome(row)).collect();
    assert_eq!(
        got,
        [
            NotMatched,
            NotEvaluated(Fault::Incompatible),
            NotEvaluated(Fault::Incompatible)
        ]
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

// The property tests below compare the executor with a reference interpreter
// that evaluates a rule on one record at a time, as ADR-0005 and ADR-0041
// define it. The conformance fixtures replace the interpreter.

/// The width of a float field, which a literal compared with the field takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Width {
    F16,
    F32,
    F64,
}

/// The value of one field of a record, as the reference interpreter reads it.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Value {
    // Every int type fits in an `i128`, so the interpreter compares ints
    // exactly.
    Int(i128),
    Float(f64, Width),
    Bool(bool),
    // The contents of a text do not count, because a comparison with a text
    // is a fault or unsupported in this version.
    Text,
    Null,
}

/// The result of an expression on one record (ADR-0005).
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

    /// The outcome of a rule whose expression gives this result.
    fn outcome(self) -> Outcome {
        match self {
            Self::True => Matched,
            Self::False | Self::Unknown => NotMatched,
            Self::Fault(f) => NotEvaluated(f),
        }
    }
}

/// The number of field `f{i}` in a generated rule, read from its one key.
fn field_number(path: &[Segment]) -> usize {
    match path {
        [Segment::Key(name)] => name[1..].parse().expect("a generated field is named f<i>"),
        _ => unreachable!("a generated path is one key"),
    }
}

/// Evaluates `e` where a condition is expected on `record`, which holds the
/// value of field `f{i}` at index `i`.
fn reference(e: &Expr, record: &[Value]) -> Tri {
    match e {
        Expr::Bool(b) => Tri::of(*b),
        // A null where a condition is expected is unknown.
        Expr::None => Tri::Unknown,
        Expr::Int(_) | Expr::Float(_) => Tri::Fault(Fault::NotBoolean),
        Expr::Not(x) => match reference(x, record) {
            Tri::True => Tri::False,
            Tri::False => Tri::True,
            other => other,
        },
        Expr::And(items) => items.iter().fold(Tri::True, |left, item| match left {
            // A false left operand decides the `and`, so the right one is not
            // evaluated and its fault does not happen.
            Tri::False | Tri::Fault(_) => left,
            _ => match reference(item, record) {
                Tri::True => left,
                right => right,
            },
        }),
        Expr::Or(items) => items.iter().fold(Tri::False, |left, item| match left {
            // A true left operand decides the `or`.
            Tri::True | Tri::Fault(_) => left,
            _ => match reference(item, record) {
                Tri::False => left,
                right => right,
            },
        }),
        Expr::Eq(l, r) => compare(l, r, record, Ordering::is_eq),
        Expr::Ne(l, r) => compare(l, r, record, Ordering::is_ne),
        Expr::Lt(l, r) => compare(l, r, record, Ordering::is_lt),
        Expr::Le(l, r) => compare(l, r, record, Ordering::is_le),
        Expr::Gt(l, r) => compare(l, r, record, Ordering::is_gt),
        Expr::Ge(l, r) => compare(l, r, record, Ordering::is_ge),
        _ => unreachable!("the generated rules hold no other kind"),
    }
}

/// Evaluates the comparison of `l` with `r` on `record`, which holds when
/// `holds` accepts the order of the two values.
fn compare(l: &Expr, r: &Expr, record: &[Value], holds: fn(Ordering) -> bool) -> Tri {
    let order = match (l, r) {
        (Expr::Field(p), literal) => order_with_literal(record[field_number(p)], literal),
        // The order of a literal with a field is the order of the field with
        // the literal, reversed.
        (literal, Expr::Field(p)) => {
            order_with_literal(record[field_number(p)], literal).map(|o| o.map(Ordering::reverse))
        }
        (a, b) => Ok(Some(order_of_literals(a, b))),
    };
    match order {
        Ok(Some(o)) => Tri::of(holds(o)),
        Ok(None) => Tri::Unknown,
        Err(fault) => Tri::Fault(fault),
    }
}

/// The order of the field value `v` and the number `literal`, `None` when
/// `v` is a null, or the fault of comparing them.
fn order_with_literal(v: Value, literal: &Expr) -> Result<Option<Ordering>, Fault> {
    let order = match (v, literal) {
        (Value::Null, _) => return Ok(None),
        (Value::Int(i), Expr::Int(j)) => i.cmp(&i128::from(*j)),
        // An int widens to a float.
        (Value::Int(i), Expr::Float(x)) => ieee(i as f64, *x),
        // A literal takes the width of the float field, so it rounds to the
        // nearest value of that width.
        (Value::Float(f, w), Expr::Int(j)) => ieee(f, int_as(*j, w)),
        (Value::Float(f, w), Expr::Float(x)) => ieee(f, float_as(*x, w)),
        (Value::Bool(_) | Value::Text, _) => return Err(Fault::Incompatible),
        _ => unreachable!("a generated literal is a number"),
    };
    Ok(Some(order))
}

/// The order of two number literals, where an int widens to a float.
fn order_of_literals(a: &Expr, b: &Expr) -> Ordering {
    match (a, b) {
        (Expr::Int(i), Expr::Int(j)) => i.cmp(j),
        _ => ieee(number(a), number(b)),
    }
}

/// The order of two floats that are no NaN, as IEEE 754 compares them, so
/// `-0.0` equals `0.0`.
fn ieee(a: f64, b: f64) -> Ordering {
    a.partial_cmp(&b).expect("a NaN is a null and no literal")
}

/// A number literal as a float.
fn number(e: &Expr) -> f64 {
    match e {
        Expr::Int(i) => *i as f64,
        Expr::Float(x) => *x,
        _ => unreachable!("a generated literal is a number"),
    }
}

/// `x` rounded to the nearest float of width `w`.
fn float_as(x: f64, w: Width) -> f64 {
    match w {
        Width::F16 => f64::from(x as f16),
        Width::F32 => f64::from(x as f32),
        Width::F64 => x,
    }
}

/// `i` rounded to the nearest float of width `w`, in one rounding, as the
/// executor converts it.
fn int_as(i: i64, w: Width) -> f64 {
    match w {
        Width::F16 => f64::from(i as f16),
        Width::F32 => f64::from(i as f32),
        Width::F64 => i as f64,
    }
}

/// The values of one generated field. A buffer of a number or a byte boolean
/// holds `step` values per row, and row `r` is the value at `r * step`.
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
    // Empty texts, one offset per row and one past the last row.
    Text(Vec<u32>),
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
    bitmap: Vec<u8>,
    mask: Vec<u8>,
}

/// A generated batch: its row count and its fields, where `None` is a field
/// that the source lacks. The rules name field `i` as `f{i}`.
#[derive(Debug, Clone)]
struct Table {
    rows: usize,
    fields: Vec<Option<Field>>,
}

/// The run of every `step`-th value of `buffer`, `rows` values long.
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
    /// The column that describes this field over its buffers.
    fn column(&self) -> Column<'_> {
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
            Values::Text(offsets) => {
                // SAFETY: the buffer is empty, which is UTF-8, and every
                // offset is 0, so each text is the empty range at its start.
                let texts = unsafe { Offsets::new(offsets, Utf8::new(&[])) };
                Column::text(Text::Offsets(AnyOffsets::U32(texts)), marking)
            }
        };
        column.expect("the marking covers the rows")
    }

    /// The value of row `row`, as the reference interpreter reads it.
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
            Values::BoolBits(bools, _) => Value::Bool(bools[row]),
            Values::Text(_) => Value::Text,
        }
    }
}

impl Table {
    /// The record at `row`, with the value of field `f{i}` at index `i`.
    fn record(&self, row: usize) -> Vec<Value> {
        self.fields
            .iter()
            .map(|f| f.as_ref().map_or(Value::Null, |f| f.value(row)))
            .collect()
    }

    /// The columns of the fields that `fields` lists, in its order.
    fn columns(&self, fields: &FieldTree) -> Vec<Option<Column<'_>>> {
        fields
            .paths()
            .iter()
            .map(|p| self.fields[field_number(p)].as_ref().map(Field::column))
            .collect()
    }
}

/// `n` ints of type `T`, mostly near 0 so that they meet the literals, and
/// some from the whole range of the type.
fn int_values<T>(n: usize) -> impl Strategy<Value = Vec<T>>
where
    T: Arbitrary + TryFrom<i64> + Default + Clone + std::fmt::Debug + 'static,
{
    let near_zero = (-6i64..6).prop_map(|i| T::try_from(i).unwrap_or_default());
    vec(prop_oneof![3 => near_zero, 1 => any::<T>()], n)
}

/// A float near the literals, a value that is no number, or any float.
fn float() -> impl Strategy<Value = f64> {
    prop_oneof![
        2 => select(vec![0.0, -0.0, 0.1, 1.0, 2.5, -3.0, 1e10, f64::NAN, f64::INFINITY]),
        1 => (-6i32..6).prop_map(f64::from),
        1 => any::<f64>(),
    ]
}

/// The values of `rows` rows of any column type that this version reads. A
/// buffer with a stride holds `step` values per row.
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
        Just(Values::Text(vec![0; rows + 1])),
    ]
}

/// A field of `rows` rows with a generated type, stride and marking.
fn field(rows: usize) -> impl Strategy<Value = Field> {
    let mark = prop_oneof![
        Just(Mark::Never),
        (0usize..8).prop_map(Mark::Bitmap),
        Just(Mark::ByteMask)
    ];
    (1usize..3, mark, vec(prop::bool::weighted(0.8), rows)).prop_flat_map(
        move |(step, mark, present)| {
            values(rows, step).prop_map(move |values| {
                // A text column and a bit-packed boolean column have no
                // stride.
                let step = if matches!(values, Values::Text(_) | Values::BoolBits(..)) {
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

/// A batch of up to `MAX_ROWS` rows, so that it crosses two chunk boundaries,
/// with one to four fields, some of which the source lacks.
fn table() -> impl Strategy<Value = Table> {
    prop_oneof![0usize..70, 0..MAX_ROWS].prop_flat_map(|rows| {
        let f = prop_oneof![6 => field(rows).prop_map(Some), 1 => Just(None)];
        vec(f, 1..5).prop_map(move |fields| Table { rows, fields })
    })
}

/// A number literal near the generated values or at the edge of a type.
fn literal() -> impl Strategy<Value = Expr> {
    prop_oneof![
        3 => (-6i64..6).prop_map(Expr::Int),
        1 => select(vec![i64::MIN, -129, 255, 256, 65504, 65520, 1 << 53, i64::MAX]).prop_map(Expr::Int),
        2 => select(vec![0.0, -0.0, 0.1, 2.5, -3.0, 1e10, 1e300]).prop_map(Expr::Float),
    ]
}

/// The field `f{i}`.
fn field_named(i: usize) -> Expr {
    Expr::Field(vec![Segment::Key(format!("f{i}"))])
}

/// An expression where a condition is expected, over `n` fields, with the
/// kinds that this version plans.
fn condition(n: usize) -> impl Strategy<Value = Expr> {
    let ops: Vec<_> = BINARY[..2].iter().chain(&NUMERIC[..4]).copied().collect();
    let comparison =
        (select(ops.clone()), 0..n, literal(), any::<bool>()).prop_map(|(op, f, lit, flip)| {
            let (f, lit) = (Box::new(field_named(f)), Box::new(lit));
            if flip { op(lit, f) } else { op(f, lit) }
        });
    let folded =
        (select(ops), literal(), literal()).prop_map(|(op, a, b)| op(Box::new(a), Box::new(b)));
    let literal_condition = prop_oneof![
        any::<bool>().prop_map(Expr::Bool),
        Just(Expr::None),
        literal()
    ];
    let leaf = prop_oneof![6 => comparison, 1 => folded, 1 => literal_condition];
    // Up to three levels of `not`, `and` and `or` over the leaves.
    leaf.prop_recursive(3, 24, 4, |inner| {
        prop_oneof![
            inner.clone().prop_map(|x| Expr::Not(Box::new(x))),
            vec(inner.clone(), 2..4).prop_map(Expr::And),
            vec(inner, 2..4).prop_map(Expr::Or),
        ]
    })
}

/// One to three rules over `n` fields.
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
        // 1. It plans the rules, binds them to the table's columns and runs
        //    them.
        // 2. It evaluates each rule on each record with the reference
        //    interpreter and checks that both give the same outcome.

        // 1. plan, bind and run the rules
        let program = plan(&rules).expect("the generated kinds plan");
        let columns = t.columns(program.fields());
        let outcomes = program.bind(&columns)?.run(t.rows, &columns)?;

        // 2. check each record against the reference
        for row in 0..t.rows {
            let record = t.record(row);
            for (rule, outcome) in rules.iter().zip(outcomes.rules()) {
                let expected = reference(&rule.expression, &record).outcome();
                prop_assert_eq!(outcome.outcome(row), expected, "{:?} at row {}", rule.expression, row);
            }
        }
    }
}
