//! Bind: it turns a `Program` and the column types of one schema into a
//! `Bound`, with one compiled loop per comparison.

use std::fmt;

use uuid::Uuid;

use super::number::{
    Converted, compare_columns, compare_widened, compare_with_literal, outside, with_order,
};
use super::outcome::Fault;
use crate::column::{
    Booleans, CHUNK_WORDS, Column, Floats, Ints, Kind, Number, Present, present_all,
    present_from_bits, present_from_mask,
};
use crate::plan::{Comparison, Constant, Literal, Operand, Program, Step};

/// A program bound to the column types of one schema. It runs over every
/// batch whose columns have those types.
pub struct Bound {
    // The type of each field that bind saw, which `run` compares with the
    // columns of each batch.
    pub(super) types: Vec<FieldType>,
    pub(super) steps: Vec<BoundStep>,
    pub(super) rules: Vec<(Uuid, usize)>,
}

/// The type of a field's column as bind sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FieldType {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F16,
    F32,
    F64,
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
    // A type fault, which faults on the rows where all `fields` have a value.
    Faulted {
        fault: Fault,
        fields: Vec<usize>,
    },
    // A null test, true where `field` has no value or, when `negated`, where
    // it has one.
    Presence {
        field: usize,
        negated: bool,
    },
    // The logic of ADR-0005 over earlier slots.
    Not(usize),
    And(usize, usize),
    Or(usize, usize),
}

/// The loop of a step. It reads the columns of a batch and writes into its
/// bitmap the true rows of `start .. start + rows`, which make one chunk.
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

/// Runs `$body` with `$T` as the int type of the field type `$t`.
macro_rules! with_int {
    ($t:expr, |$T:ident| $body:expr) => {
        match $t {
            FieldType::I8 => {
                type $T = i8;
                $body
            }
            FieldType::I16 => {
                type $T = i16;
                $body
            }
            FieldType::I32 => {
                type $T = i32;
                $body
            }
            FieldType::I64 => {
                type $T = i64;
                $body
            }
            FieldType::U8 => {
                type $T = u8;
                $body
            }
            FieldType::U16 => {
                type $T = u16;
                $body
            }
            FieldType::U32 => {
                type $T = u32;
                $body
            }
            FieldType::U64 => {
                type $T = u64;
                $body
            }
            _ => unreachable!("the caller checked that the field holds ints"),
        }
    };
}

/// Runs `$body` with `$T` as the number type of the field type `$t`.
macro_rules! with_number {
    ($t:expr, |$T:ident| $body:expr) => {
        if $t.is_int() {
            with_int!($t, |$T| $body)
        } else {
            with_float!($t, |$T| $body)
        }
    };
}

/// Runs `$body` with `$T` as the float type of the field type `$t`.
macro_rules! with_float {
    ($t:expr, |$T:ident| $body:expr) => {
        match $t {
            FieldType::F16 => {
                type $T = f16;
                $body
            }
            FieldType::F32 => {
                type $T = f32;
                $body
            }
            FieldType::F64 => {
                type $T = f64;
                $body
            }
            _ => unreachable!("the caller checked that the field holds floats"),
        }
    };
}

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
        Step::IsNull { field, negated } if types[*field] == FieldType::Absent => {
            BoundStep::Constant(Constant::of(!*negated))
        }
        Step::IsNull { field, negated } => BoundStep::Presence {
            field: *field,
            negated: *negated,
        },
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
        // The planner puts a field on the left of every comparison.
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
        // `x == False` holds where `x` is false, and `x != True` too.
        (FieldType::Bool, Literal::Bool(b)) => BoundStep::Loop {
            fields: vec![f],
            chunk: booleans_equal(f, b == (op == Comparison::Eq)),
        },
        (_, Literal::Bool(_)) | (FieldType::Bool | FieldType::Text | FieldType::List, _) => {
            BoundStep::Faulted {
                fault: Fault::Incompatible,
                fields: vec![f],
            }
        }
        // An int widens to a float (ADR-0041), so the column's ints widen to
        // the float literal.
        (_, Literal::Float(x)) if t.is_int() => {
            with_int!(t, |T| widened::<T>(op, f, x))
        }
        (_, Literal::Int(i)) if t.is_int() => with_int!(t, |T| int_literal::<T>(op, f, i)),
        // The literal, int or float, converts to the column's float type.
        (_, literal) => with_float!(t, |T| float_literal::<T>(op, f, literal)),
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
        (FieldType::Absent, _) | (_, FieldType::Absent) => BoundStep::Constant(Constant::Unknown),
        // The macros write one arm per pair of number types, so the compiler
        // instantiates one loop per pair and operator.
        _ if lt.is_number() && rt.is_number() => {
            let chunk: ChunkLoop =
                with_number!(lt, |L| with_number!(rt, |R| with_order!(op, |O| {
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
        Converted::Below => constant_loop(outside(op, false)),
        Converted::Above => constant_loop(outside(op, true)),
    };
    BoundStep::Loop {
        fields: vec![f],
        chunk,
    }
}

/// The step that compares float field `f` with `literal`, converted to the
/// field's type. A literal beyond the type's range converts to an infinity.
fn float_literal<T: Number>(op: Comparison, f: usize, literal: Literal) -> BoundStep {
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
        // A bit-packed column holds its values as a bitmap of the chunk does.
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
            Kind::Ints(Ints::I8(_), _) => Self::I8,
            Kind::Ints(Ints::I16(_), _) => Self::I16,
            Kind::Ints(Ints::I32(_), _) => Self::I32,
            Kind::Ints(Ints::I64(_), _) => Self::I64,
            Kind::Ints(Ints::U8(_), _) => Self::U8,
            Kind::Ints(Ints::U16(_), _) => Self::U16,
            Kind::Ints(Ints::U32(_), _) => Self::U32,
            Kind::Ints(Ints::U64(_), _) => Self::U64,
            Kind::Floats(Floats::F16(_), _) => Self::F16,
            Kind::Floats(Floats::F32(_), _) => Self::F32,
            Kind::Floats(Floats::F64(_), _) => Self::F64,
            Kind::Booleans(..) => Self::Bool,
            Kind::Text(..) => Self::Text,
            Kind::List(..) => Self::List,
        }
    }

    /// Whether the field holds numbers.
    fn is_number(self) -> bool {
        self.is_int() || matches!(self, Self::F16 | Self::F32 | Self::F64)
    }

    /// Whether the field holds ints.
    fn is_int(self) -> bool {
        matches!(
            self,
            Self::I8
                | Self::I16
                | Self::I32
                | Self::I64
                | Self::U8
                | Self::U16
                | Self::U32
                | Self::U64
        )
    }
}
