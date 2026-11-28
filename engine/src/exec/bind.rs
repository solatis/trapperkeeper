//! Bind: it turns a `Program` and the column types of one schema into a
//! `Bound`, with one compiled loop per comparison.

use std::fmt;

use uuid::Uuid;

use super::number::{
    Converted, compare_columns, compare_widened, compare_with_literal, outside, with_order,
};
use super::outcome::Fault;
use crate::column::{
    Booleans, CHUNK_WORDS, Column, Kind, Number, NumberType, Present, present_all,
    present_from_bits, present_from_mask, with_float, with_int, with_number,
};
use crate::plan::{Comparison, Constant, Literal, Operand, Program, Step};

/// A program bound to the column types of one schema. It runs over every
/// batch whose columns have those types.
pub struct Bound {
    // The type of each field that bind saw, which `run` compares with the
    // columns of each batch.
    pub(super) types: Vec<FieldType>,
    pub(super) steps: Vec<BoundStep>,
    // The id of each rule with the slot that holds its result, from the
    // program.
    pub(super) rules: Vec<(Uuid, usize)>,
}

/// The type of a field's column as bind sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FieldType {
    Number(NumberType),
    Bool,
    Text,
    List,
    // A field that the source lacks.
    Absent,
}

/// A step of a program with the loop that bind picked for it.
pub(crate) enum BoundStep {
    Constant(Constant),
    // A comparison whose loop writes the true rows. A row is known where all
    // `fields` have a value.
    Loop {
        fields: Vec<usize>,
        chunk: ChunkLoop,
    },
    // A type fault, which faults on the rows where all `fields` have a value
    // and is unknown on the others.
    Faulted {
        fault: Fault,
        fields: Vec<usize>,
    },
    // `x is None` for the field with this number: true on the rows where the
    // field has no value.
    IsNull(usize),
    // The logic of ADR-0005 over the results of earlier steps, by their
    // slots.
    Not(usize),
    And(usize, usize),
    Or(usize, usize),
}

/// The loop of a step. It takes the columns of a batch, the first row `start`
/// of a chunk and its row count `rows`, and writes into its bitmap the true
/// rows of the chunk, with row `start` at bit 0.
pub(crate) type ChunkLoop =
    Box<dyn Fn(&[Option<Column<'_>>], usize, usize, &mut Present) + Send + Sync>;

/// Why `bind` refused a program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindError {
    /// The columns are not one per field of the program.
    Columns { expected: usize, given: usize },
    /// A comparison of two fields whose types this version does not compare,
    /// such as two text fields.
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
    /// Binds the program to the types of `columns`, one per field in the order
    /// of `FieldTree::paths`, where `None` is a field that the source lacks.
    ///
    /// # Errors
    ///
    /// `BindError::Columns` when the columns are not one per field, and
    /// `BindError::Unsupported` for a comparison of two fields whose types
    /// this version does not compare.
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

/// Binds `step` to the field types `types`. Fails for a comparison of two
/// fields whose types this version does not compare.
fn bind_step(step: &Step, types: &[FieldType]) -> Result<BoundStep, BindError> {
    Ok(match step {
        Step::Constant(c) => BoundStep::Constant(*c),
        Step::Condition(f) => bind_condition(*f, types[*f]),
        // A field that the source lacks is a null on every row.
        Step::IsNull(f) if types[*f] == FieldType::Absent => BoundStep::Constant(Constant::True),
        Step::IsNull(f) => BoundStep::IsNull(*f),
        Step::Not(a) => BoundStep::Not(*a),
        Step::And(a, b) => BoundStep::And(*a, *b),
        Step::Or(a, b) => BoundStep::Or(*a, *b),
        Step::Compare {
            op,
            left: Operand::Field(f),
            right: Operand::Literal(literal),
        } => bind_literal(*op, *f, types[*f], *literal),
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

/// Binds field `f`, of type `t`, where a condition is expected.
fn bind_condition(f: usize, t: FieldType) -> BoundStep {
    match t {
        // A null where a condition is expected is unknown (ADR-0005).
        FieldType::Absent => BoundStep::Constant(Constant::Unknown),
        FieldType::Bool => BoundStep::Loop {
            fields: vec![f],
            chunk: booleans_equal(f, true),
        },
        _ => BoundStep::Faulted {
            fault: Fault::NotBoolean,
            fields: vec![f],
        },
    }
}

/// Binds a comparison of field `f`, of type `t`, with `literal`.
fn bind_literal(op: Comparison, f: usize, t: FieldType, literal: Literal) -> BoundStep {
    // The function tests the pair of the field's type and the literal in a
    // chain, and the first case that applies gives the step. The absent field
    // and the boolean cases come before the number cases, because the number
    // cases take every other pair.
    match (t, literal) {
        // A field without a value on any row makes the comparison unknown on
        // every row (ADR-0005).
        (FieldType::Absent, _) => BoundStep::Constant(Constant::Unknown),
        // An ordering is for numbers only (ADR-0041).
        (FieldType::Bool, Literal::Bool(_)) if op.is_ordering() => BoundStep::Faulted {
            fault: Fault::Unordered,
            fields: vec![f],
        },
        // The arm before takes the orderings, so `op` is `==` or `!=` here.
        // `x == False` holds where `x` is false, and `x != True` too.
        (FieldType::Bool, Literal::Bool(b)) => BoundStep::Loop {
            fields: vec![f],
            chunk: booleans_equal(f, b == (op == Comparison::Eq)),
        },
        // A boolean literal against a number, a text or a list field, and a
        // number literal against a boolean, a text or a list field, compare
        // values of types that do not compare.
        (_, Literal::Bool(_)) | (FieldType::Bool | FieldType::Text | FieldType::List, _) => {
            BoundStep::Faulted {
                fault: Fault::Incompatible,
                fields: vec![f],
            }
        }
        // An int widens to a float (ADR-0041), so the column's ints widen to
        // the float literal.
        (FieldType::Number(n), Literal::Float(x)) if !n.is_float() => {
            with_int!(n, |T| widened::<T>(op, f, x))
        }
        (FieldType::Number(n), Literal::Int(i)) if !n.is_float() => {
            with_int!(n, |T| int_literal::<T>(op, f, i))
        }
        // The literal, int or float, converts to the column's float type.
        (FieldType::Number(n), literal) => {
            with_float!(n, |T| float_literal::<T>(op, f, literal))
        }
    }
}

/// Binds a comparison of field `l`, of type `lt`, with field `r`, of type
/// `rt`. Fails for two text fields or two list fields, which this version
/// does not compare.
fn bind_fields(
    op: Comparison,
    l: usize,
    lt: FieldType,
    r: usize,
    rt: FieldType,
) -> Result<BoundStep, BindError> {
    let fields = vec![l, r];
    Ok(match (lt, rt) {
        // A field without a value on any row makes the comparison unknown on
        // every row (ADR-0005), whatever the other field holds, so this arm
        // comes before the arms that fault or refuse a pair of types.
        (FieldType::Absent, _) | (_, FieldType::Absent) => BoundStep::Constant(Constant::Unknown),
        // The macros write one arm per pair of number types, so the compiler
        // instantiates one loop per pair and operator.
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
        // An ordering is for numbers only (ADR-0041).
        (FieldType::Bool, FieldType::Bool) if op.is_ordering() => BoundStep::Faulted {
            fault: Fault::Unordered,
            fields,
        },
        (FieldType::Bool, FieldType::Bool) => {
            let equal = op == Comparison::Eq;
            let chunk: ChunkLoop = Box::new(
                move |cols: &[Option<Column<'_>>], start: usize, rows: usize, out: &mut Present| {
                    let mut right = [0; CHUNK_WORDS];
                    write_booleans(cols, l, start, rows, out);
                    write_booleans(cols, r, start, rows, &mut right);
                    // A set bit marks the rows whose two booleans differ.
                    for (word, other) in out.iter_mut().zip(right) {
                        *word ^= other;
                    }
                    if equal {
                        invert(out, rows);
                    }
                },
            );
            BoundStep::Loop { fields, chunk }
        }
        (FieldType::Text, FieldType::Text) | (FieldType::List, FieldType::List) => {
            return Err(BindError::Unsupported { left: l, right: r });
        }
        // Any other pair, such as a number and a text or a boolean and a
        // number, holds types that do not compare (ADR-0005).
        _ => BoundStep::Faulted {
            fault: Fault::Incompatible,
            fields,
        },
    })
}

/// The step that compares int field `f` with the literal `i`, converted to the
/// field's type.
fn int_literal<T: Number + TryFrom<i64>>(op: Comparison, f: usize, i: i64) -> BoundStep {
    let chunk: ChunkLoop = match Converted::<T>::of(i) {
        Converted::Within(literal) => with_order!(op, |O| Box::new(
            move |cols: &[Option<Column<'_>>], start: usize, rows: usize, out: &mut Present| {
                compare_with_literal::<T, O>(values_of(cols, f), literal, start, rows, out);
            }
        )),
        // A literal outside the type decides the comparison for every value.
        Converted::Outside(side) => constant_loop(outside(op, side)),
    };
    BoundStep::Loop {
        fields: vec![f],
        chunk,
    }
}

/// The step that compares float field `f` with `literal`, converted to the
/// field's type. A literal beyond the type's range converts to an infinity.
fn float_literal<T: Number>(op: Comparison, f: usize, literal: Literal) -> BoundStep {
    // The literal rounds to the column's type, so a value near the literal
    // compares with the rounded literal. On an `f16` column, `x > 0.1`
    // compares with the `f16` nearest to 0.1 (ADR-0041).
    let literal: T = match literal {
        Literal::Int(i) => convert(i),
        Literal::Float(x) => convert(x),
        Literal::Bool(_) => unreachable!("`bind_literal` faults a boolean against a number"),
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

/// The step that compares int field `f`, widened to `f64`, with the float `x`.
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

/// The loop that writes the rows of boolean field `f` that hold `value`.
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

/// Writes into `out` the rows `start .. start + rows` of boolean field `f`
/// that are true.
fn write_booleans(
    cols: &[Option<Column<'_>>],
    f: usize,
    start: usize,
    rows: usize,
    out: &mut Present,
) {
    let booleans = match cols[f].as_ref().map(Column::kind) {
        Some(Kind::Booleans(b, _)) => *b,
        // `run` checks each column against the type that bind saw, and bind
        // made this loop for a boolean field.
        _ => unreachable!("run checked the type of the column"),
    };
    match booleans {
        // A bit-packed boolean column has the bit order of a chunk's bitmap,
        // so the loop of a bitmap copies its true rows.
        Booleans::Bits(bits) => present_from_bits(bits, start, rows, out),
        // The loop of a byte mask sets the rows whose byte is zero, which are
        // the false rows here.
        Booleans::Bytes(bytes) => {
            present_from_mask(bytes, start, rows, out);
            invert(out, rows);
        }
    }
}

/// Swaps the set and the clear bits of the first `rows` rows of `out`, and
/// leaves the bits from `rows` on clear.
fn invert(out: &mut Present, rows: usize) {
    let mut all = [0; CHUNK_WORDS];
    present_all(rows, &mut all);
    for (word, held) in out.iter_mut().zip(all) {
        *word = !*word & held;
    }
}

/// `value` converted to `C` as Rust's `as` converts it.
fn convert<F: Number, C: Number>(value: F) -> C {
    // The sealed trait that holds `convert` cannot be named outside
    // `column`, so a call on a concrete type finds no method. The bound
    // `F: Number` brings it into reach.
    value.convert()
}

/// The loop that writes `holds` for every row of a chunk.
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

/// The values of field `f` among `cols`, as a run of `T`.
fn values_of<'a, T: Number>(
    cols: &[Option<Column<'a>>],
    f: usize,
) -> crate::column::Strided<'a, T> {
    cols[f]
        .as_ref()
        .and_then(|c| T::values(c.kind()))
        // `run` checks each column against the type that bind saw, and bind
        // picked `T` from that type.
        .expect("run checked the type of the column")
}

impl FieldType {
    /// The type of `c`, or `Absent` for `None`.
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
