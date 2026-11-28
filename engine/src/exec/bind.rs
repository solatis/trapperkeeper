//! Bind: specializes a program for the column types of one schema.
//!
//! The rule language is untyped, and so is a `Program`: `x > 5` says nothing
//! about whether `x` is an `i8` or an `f64`. The Rust code that compares
//! values must know the type, because each type has its own SIMD
//! instructions. Bind looks at the actual type of each column and, for each
//! comparison, picks a loop that was compiled for exactly that type. For an
//! `i32` column `x`, `x > 5` gets the `i32` loop, with 5 stored as an `i32`.
//!
//! Bind also decides everything else that depends only on the types, once
//! per schema instead of once per batch. It decides which comparisons fault,
//! such as an ordering on a text field, and which pairs of types can never be
//! equal, such as a number and a text. It also decides how a literal converts
//! to the column's type, and which literals no value of the column can reach.

use std::fmt;
use std::sync::Arc;

use uuid::Uuid;

use super::number::{
    Converted, compare_columns, compare_widened, compare_with_literal, outside, with_order,
};
use super::outcome::Fault;
use super::text;
use crate::column::{
    Booleans, CHUNK_WORDS, Column, Kind, Number, NumberType, Present, present_all,
    present_from_bits, present_from_mask, with_float, with_int, with_number,
};
use crate::plan::{Comparison, Constant, Literal, Operand, Program, Step};

/// A program specialized for the column types of one schema. `Bound::run`
/// runs it over any batch whose columns have those types.
pub struct Bound {
    // The column type of each field at bind. `run` checks each batch against
    // these, because the loops read a column's memory as this type.
    pub(super) types: Vec<FieldType>,
    pub(super) steps: Vec<BoundStep>,
    // For each rule: its id and the slot that holds its result, copied from
    // the program.
    pub(super) rules: Vec<(Uuid, usize)>,
}

/// The type of a field's column, which decides how bind compiles each step
/// that reads the field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FieldType {
    Number(NumberType),
    Bool,
    Text,
    List,
    // The source has no column for the field, so it has no value on any
    // row.
    Absent,
}

/// A step of a program after bind: each comparison now holds the loop that
/// computes it.
pub(crate) enum BoundStep {
    Constant(Constant),
    // A comparison and its loop. The loop writes the rows where the
    // comparison is true. On a row where one of `fields` has no value, the
    // result is unknown whatever the loop wrote.
    Loop {
        fields: Vec<usize>,
        chunk: ChunkLoop,
    },
    // A comparison that cannot be done, such as an ordering on text. It is a
    // fault on the rows where all `fields` have a value, and unknown on the
    // rows where one of them has none (ADR-0005).
    Faulted {
        fault: Fault,
        fields: Vec<usize>,
    },
    // `x is None` for the field with this number: true on the rows where the
    // field has no value.
    IsNull(usize),
    // `not`, `and` and `or` over the results of earlier steps, by their
    // slots, as in `Step`.
    Not(usize),
    And(usize, usize),
    Or(usize, usize),
}

/// The compiled loop of a comparison. A call gets the columns of a batch,
/// the first row `start` of a chunk and the chunk's row count `rows`. It
/// writes a bitmap with one bit per row of the chunk, set where the
/// comparison is true, with row `start` at bit 0.
pub(crate) type ChunkLoop =
    Box<dyn Fn(&[Option<Column<'_>>], usize, usize, &mut Present) + Send + Sync>;

/// The error of `bind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindError {
    /// The caller passed a different number of columns than the program has
    /// fields.
    Columns { expected: usize, given: usize },
    /// The program compares two fields whose types this version cannot
    /// compare yet, which are two list fields.
    Unsupported { left: usize, right: usize },
}

impl fmt::Display for BindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Columns { expected, given } => {
                write!(
                    f,
                    "the program reads {expected} fields and got {given} columns"
                )
            }
            Self::Unsupported { left, right } => write!(
                f,
                "this version does not compare the types of fields {left} and {right}"
            ),
        }
    }
}

impl std::error::Error for BindError {}

impl Program {
    /// Specializes the program for the types of `columns`. `columns` holds
    /// one entry per field, in the order of `FieldTree::paths`, and `None`
    /// for a field that the source does not have. Bind reads only the types
    /// of the columns, so any batch of the same schema can stand in.
    ///
    /// # Errors
    ///
    /// `BindError::Columns` when `columns` does not have one entry per field,
    /// and `BindError::Unsupported` when the program compares two fields
    /// whose types this version cannot compare yet.
    pub fn bind(&self, columns: &[Option<Column<'_>>]) -> Result<Bound, BindError> {
        let expected = self.fields().paths().len();
        if columns.len() != expected {
            return Err(BindError::Columns {
                expected,
                given: columns.len(),
            });
        }
        let types: Vec<FieldType> = columns.iter().map(|c| FieldType::of(c.as_ref())).collect();
        let steps = self
            .steps()
            .iter()
            .map(|s| bind_step(s, &types))
            .collect::<Result<_, _>>()?;
        Ok(Bound {
            types,
            steps,
            rules: self.rules().to_vec(),
        })
    }
}

/// Specializes one step for the field types `types`. Fails when the step
/// compares two fields whose types this version cannot compare yet.
fn bind_step(step: &Step, types: &[FieldType]) -> Result<BoundStep, BindError> {
    Ok(match step {
        Step::Constant(c) => BoundStep::Constant(*c),
        Step::Condition(f) => bind_condition(*f, types[*f]),
        // A field without a column has no value on any row, so `x is None`
        // is true everywhere.
        Step::IsNull(f) if types[*f] == FieldType::Absent => BoundStep::Constant(Constant::True),
        Step::IsNull(f) => BoundStep::IsNull(*f),
        Step::Not(a) => BoundStep::Not(*a),
        Step::And(a, b) => BoundStep::And(*a, *b),
        Step::Or(a, b) => BoundStep::Or(*a, *b),
        Step::Compare {
            op,
            left: Operand::Field(f),
            right: Operand::Literal(literal),
        } => bind_literal(*op, *f, types[*f], literal),
        Step::Compare {
            op,
            left: Operand::Field(l),
            right: Operand::Field(r),
        } => bind_fields(*op, *l, types[*l], *r, types[*r])?,
        Step::Compare {
            left: Operand::Literal(_),
            ..
        } => unreachable!("the planner holds a field on the left"),
    })
}

/// Specializes a field `f` of type `t` that is used on its own as a
/// condition, as `x` is in `x and y`.
fn bind_condition(f: usize, t: FieldType) -> BoundStep {
    match t {
        // A missing field is null on every row, and a null used as a
        // condition is unknown (ADR-0005).
        FieldType::Absent => BoundStep::Constant(Constant::Unknown),
        // A boolean field as a condition means `x == True`.
        FieldType::Bool => BoundStep::Loop {
            fields: vec![f],
            chunk: booleans_equal(f, true),
        },
        // A number, text or list as a condition is a fault (ADR-0041).
        _ => BoundStep::Faulted {
            fault: Fault::NotBoolean,
            fields: vec![f],
        },
    }
}

/// Specializes `x op literal`, where `x` is field `f` and its column has
/// type `t`.
fn bind_literal(op: Comparison, f: usize, t: FieldType, literal: &Literal) -> BoundStep {
    // The arms test the pair (column type, literal) from the most specific
    // case to the least. The missing field, the orderings that fault, the
    // booleans, the texts and the pairs that can never be equal come first,
    // because the number arms at the end accept any number column with any
    // literal that reaches them.
    match (t, literal) {
        // A missing field is null on every row, and a comparison with a null
        // is unknown (ADR-0005).
        (FieldType::Absent, _) => BoundStep::Constant(Constant::Unknown),
        // `x < True`, `x < "a"`, and `x > 5` on a text column, are a fault,
        // because only numbers can be ordered (ADR-0041).
        (FieldType::Bool | FieldType::Text | FieldType::List, _)
        | (_, Literal::Bool(_) | Literal::Text(_))
            if op.is_ordering() =>
        {
            BoundStep::Faulted {
                fault: Fault::Unordered,
                fields: vec![f],
            }
        }
        // The loop marks the rows where `x` holds the boolean that makes the
        // comparison true: `x == True` and `x != False` look for true, and
        // `x == False` and `x != True` look for false.
        (FieldType::Bool, Literal::Bool(b)) => BoundStep::Loop {
            fields: vec![f],
            chunk: booleans_equal(f, *b == (op == Comparison::Eq)),
        },
        (FieldType::Text, Literal::Text(text)) => BoundStep::Loop {
            fields: vec![f],
            chunk: text::equals_literal(f, Arc::clone(text), op == Comparison::Eq),
        },
        // A pair that can never be equal, such as `x == True` on a number
        // column or `x == 5` on a text column. `==` is false and `!=` true on
        // every row with a value, as Python compares an int with a text.
        (_, Literal::Bool(_) | Literal::Text(_))
        | (FieldType::Bool | FieldType::Text | FieldType::List, _) => BoundStep::Loop {
            fields: vec![f],
            chunk: constant_loop(op == Comparison::Ne),
        },
        // An int column against a float literal, as in `x > 2.5`: each int
        // converts to `f64`, because an int compared with a float compares
        // as a float (ADR-0041). Converting 2.5 to an int instead would make
        // `x == 2.5` true where `x` is 2.
        (FieldType::Number(n), &Literal::Float(x)) if !n.is_float() => {
            with_int!(n, |T| widened::<T>(op, f, x))
        }
        // An int column against an int literal, as in `x > 5`: the literal
        // converts to the column's type once, here.
        (FieldType::Number(n), &Literal::Int(i)) if !n.is_float() => {
            with_int!(n, |T| int_literal::<T>(op, f, i))
        }
        // A float column against any number literal: the literal converts
        // to the column's float type once, here, and the loop compares in
        // that type.
        (FieldType::Number(n), literal) => {
            with_float!(n, |T| float_literal::<T>(op, f, literal))
        }
    }
}

/// Specializes `x op y`, where `x` is field `l` with column type `lt` and `y`
/// is field `r` with column type `rt`. Fails for two list fields, which this
/// version cannot compare yet.
fn bind_fields(
    op: Comparison,
    l: usize,
    lt: FieldType,
    r: usize,
    rt: FieldType,
) -> Result<BoundStep, BindError> {
    let fields = vec![l, r];
    Ok(match (lt, rt) {
        // A missing field makes the comparison unknown on every row
        // (ADR-0005), whatever type the other field has. So this arm comes
        // first, before the arms that fault or refuse a pair of types.
        (FieldType::Absent, _) | (_, FieldType::Absent) => BoundStep::Constant(Constant::Unknown),
        // Two number columns. `ln` and `rn` are only known at run time, but
        // the loop must be compiled for concrete types. `with_number!`
        // expands into a `match` with one arm per number type and sets `L`
        // to that type in its arm, `with_number!` again does the same for
        // `R`, and `with_order!` does it for the operator `O`. So this
        // expression is a `match` over all 11 x 11 x 6 = 726 combinations,
        // and each arm holds a closure that calls `compare_columns` with its
        // own `L`, `R` and `O`. The compiler builds one SIMD loop per arm,
        // and the arm that matches the actual types is the closure that bind
        // keeps. `run` then calls it once per chunk.
        (FieldType::Number(ln), FieldType::Number(rn)) => {
            let chunk: ChunkLoop =
                with_number!(ln, |L| with_number!(rn, |R| with_order!(op, |O| {
                    Box::new(
                        move |cols: &[Option<Column<'_>>],
                              start: usize,
                              rows: usize,
                              out: &mut Present| {
                            compare_columns::<L, R, O>(
                                values_of(cols, l),
                                values_of(cols, r),
                                start,
                                rows,
                                out,
                            );
                        },
                    )
                })));
            BoundStep::Loop { fields, chunk }
        }
        // `x < y` on two boolean columns is a fault, because only numbers
        // can be ordered (ADR-0041).
        (FieldType::Bool, FieldType::Bool) if op.is_ordering() => BoundStep::Faulted {
            fault: Fault::Unordered,
            fields,
        },
        // `x == y` or `x != y` on two boolean columns. The loop reads both
        // columns as bitmaps and compares 64 rows per instruction.
        (FieldType::Bool, FieldType::Bool) => {
            let equal = op == Comparison::Eq;
            let chunk: ChunkLoop = Box::new(
                move |cols: &[Option<Column<'_>>], start: usize, rows: usize, out: &mut Present| {
                    let mut right = [0; CHUNK_WORDS];
                    write_booleans(cols, l, start, rows, out);
                    write_booleans(cols, r, start, rows, &mut right);
                    // XOR sets the bit of each row where `x` and `y` differ,
                    // which is the result of `x != y`.
                    for (word, other) in out.iter_mut().zip(right) {
                        *word ^= other;
                    }
                    // `x == y` is the opposite.
                    if equal {
                        invert(out, rows);
                    }
                },
            );
            BoundStep::Loop { fields, chunk }
        }
        // `x == y` or `x != y` on two text columns, in any two layouts.
        (FieldType::Text, FieldType::Text) if !op.is_ordering() => BoundStep::Loop {
            fields,
            chunk: text::fields_equal(l, r, op == Comparison::Eq),
        },
        (FieldType::List, FieldType::List) => {
            return Err(BindError::Unsupported { left: l, right: r });
        }
        // An ordering of any other pair, such as two texts or a number and a
        // text, is a fault, because one side is not a number (ADR-0041).
        _ if op.is_ordering() => BoundStep::Faulted {
            fault: Fault::Unordered,
            fields,
        },
        // The pairs left under `==` and `!=`, such as a number and a text,
        // can never be equal, so `==` is false and `!=` true.
        _ => BoundStep::Loop {
            fields,
            chunk: constant_loop(op == Comparison::Ne),
        },
    })
}

/// Builds the step for `x op i`, where `x` is field `f`, its column holds the
/// int type `T`, and `i` is an int literal.
fn int_literal<T: Number + TryFrom<i64>>(op: Comparison, f: usize, i: i64) -> BoundStep {
    let chunk: ChunkLoop = match Converted::<T>::of(i) {
        // `with_order!` turns the operator into a type `O`, so the loop is
        // compiled for that operator.
        Converted::Within(literal) => with_order!(op, |O| Box::new(
            move |cols: &[Option<Column<'_>>], start: usize, rows: usize, out: &mut Present| {
                compare_with_literal::<T, O>(values_of(cols, f), literal, start, rows, out);
            }
        )),
        // When `T` cannot hold the literal, as in `x < 300` on a `u8`
        // column, every value of the column lies on the same side of it, so
        // the result is the same on every row and no values are read.
        Converted::Outside(side) => constant_loop(outside(op, side)),
    };
    BoundStep::Loop {
        fields: vec![f],
        chunk,
    }
}

/// Builds the step for `x op literal`, where `x` is field `f` and its column
/// holds the float type `T`. A literal too large for `T` becomes an
/// infinity, which still compares correctly with every finite value.
fn float_literal<T: Number>(op: Comparison, f: usize, literal: &Literal) -> BoundStep {
    // The literal rounds to the nearest value of `T`, and the loop compares
    // with that. On an `f16` column, `x > 0.1` compares with the `f16`
    // closest to 0.1 (ADR-0041).
    let literal: T = match *literal {
        Literal::Int(i) => convert(i),
        Literal::Float(x) => convert(x),
        Literal::Bool(_) | Literal::Text(_) => {
            unreachable!("`bind_literal` decides a boolean or a text against a number")
        }
    };
    let chunk: ChunkLoop = with_order!(op, |O| Box::new(
        move |cols: &[Option<Column<'_>>], start: usize, rows: usize, out: &mut Present| {
            compare_with_literal::<T, O>(values_of(cols, f), literal, start, rows, out);
        }
    ));
    BoundStep::Loop {
        fields: vec![f],
        chunk,
    }
}

/// Builds the step for `x op literal`, where `x` is field `f`, its column
/// holds the int type `T`, and the literal is the float `x`. Each int
/// converts to `f64` before the comparison.
fn widened<T: Number>(op: Comparison, f: usize, x: f64) -> BoundStep {
    let chunk: ChunkLoop = with_order!(op, |O| Box::new(
        move |cols: &[Option<Column<'_>>], start: usize, rows: usize, out: &mut Present| {
            compare_widened::<T, O>(values_of(cols, f), x, start, rows, out);
        }
    ));
    BoundStep::Loop {
        fields: vec![f],
        chunk,
    }
}

/// Builds the loop that marks the rows where boolean field `f` equals
/// `value`.
fn booleans_equal(f: usize, value: bool) -> ChunkLoop {
    Box::new(
        move |cols: &[Option<Column<'_>>], start: usize, rows: usize, out: &mut Present| {
            write_booleans(cols, f, start, rows, out);
            if !value {
                invert(out, rows);
            }
        },
    )
}

/// Sets the bit in `out` of each row in `start .. start + rows` where the
/// boolean field `f` is true.
fn write_booleans(
    cols: &[Option<Column<'_>>],
    f: usize,
    start: usize,
    rows: usize,
    out: &mut Present,
) {
    let booleans = match cols[f].as_ref().map(Column::kind) {
        Some(Kind::Booleans(b, _)) => *b,
        // Bind made this loop for a boolean column, and `run` refuses a
        // batch whose column has another type.
        _ => unreachable!("run checked the type of the column"),
    };
    match booleans {
        // Bit-packed booleans, as Arrow stores them, have the same layout as
        // our bitmaps, so the bits are copied as they are.
        Booleans::Bits(bits) => present_from_bits(bits, start, rows, out),
        // One byte per boolean, as numpy stores them. `present_from_mask`
        // sets the bit of each row whose byte is zero, which here are the
        // false rows, so the bits are inverted afterwards.
        Booleans::Bytes(bytes) => {
            present_from_mask(bytes, start, rows, out);
            invert(out, rows);
        }
    }
}

/// Flips the bits of the first `rows` rows of `out`. The bits from `rows`
/// on stay clear, because they are past the end of the chunk.
fn invert(out: &mut Present, rows: usize) {
    let mut all = [0; CHUNK_WORDS];
    present_all(rows, &mut all);
    for (word, held) in out.iter_mut().zip(all) {
        *word = !*word & held;
    }
}

/// Converts `value` to `C` the way Rust's `as` would.
fn convert<F: Number, C: Number>(value: F) -> C {
    // This wrapper exists because `convert` sits on a private trait of
    // `column`. Called on a concrete type such as `i64`, the method is not
    // found here. Called through the bound `F: Number`, it is.
    value.convert()
}

/// Builds a loop that ignores the columns and writes the same result,
/// `holds`, for every row.
fn constant_loop(holds: bool) -> ChunkLoop {
    Box::new(
        move |_: &[Option<Column<'_>>], _: usize, rows: usize, out: &mut Present| {
            if holds {
                present_all(rows, out);
            } else {
                out.fill(0);
            }
        },
    )
}

/// Returns the values of field `f`, read as type `T`.
fn values_of<'a, T: Number>(
    cols: &[Option<Column<'a>>],
    f: usize,
) -> crate::column::Strided<'a, T> {
    cols[f]
        .as_ref()
        .and_then(|c| T::values(c.kind()))
        // Bind picked `T` from the column's type, and `run` refuses a batch
        // whose column has another type.
        .expect("run checked the type of the column")
}

impl FieldType {
    /// Returns the type of column `c`, or `Absent` when there is no column.
    pub(crate) fn of(c: Option<&Column<'_>>) -> Self {
        let Some(c) = c else {
            return Self::Absent;
        };
        match c.kind() {
            Kind::Ints(v, _) => Self::Number(v.number_type()),
            Kind::Floats(v, _) => Self::Number(v.number_type()),
            Kind::Booleans(..) => Self::Bool,
            Kind::Text(..) => Self::Text,
            Kind::List(..) => Self::List,
        }
    }
}
