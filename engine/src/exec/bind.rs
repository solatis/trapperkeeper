//! Bind: it turns a `Program` and the column types of one schema into a
//! `Bound`, with one compiled loop per comparison.

use std::fmt;

use uuid::Uuid;

use super::number::{Converted, compare_widened, compare_with_literal, outside, with_order};
use super::outcome::Fault;
use crate::column::{Booleans, Column, Floats, Ints, Kind, Number, Present, present_all};
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
    BoolBytes,
    BoolBits,
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
    /// `BindError::Columns` when the columns are not one per field.
    pub fn bind(&self, columns: &[Option<Column<'_>>]) -> Result<Bound, BindError> {
        let expected = self.fields().paths().len();
        if columns.len() != expected {
            return Err(BindError::Columns {
                expected,
                given: columns.len(),
            });
        }
        let types: Vec<FieldType> = columns.iter().map(|c| FieldType::of(c.as_ref())).collect();
        let steps = self.steps().iter().map(|s| bind_step(s, &types)).collect();
        Ok(Bound {
            types,
            steps,
            rules: self.rules().to_vec(),
        })
    }
}

/// Binds `step` to the field types `types`.
fn bind_step(step: &Step, types: &[FieldType]) -> BoundStep {
    match step {
        Step::Constant(c) => BoundStep::Constant(*c),
        Step::Not(a) => BoundStep::Not(*a),
        Step::And(a, b) => BoundStep::And(*a, *b),
        Step::Or(a, b) => BoundStep::Or(*a, *b),
        Step::Compare {
            op,
            left: Operand::Field(f),
            right: Operand::Literal(literal),
        } => bind_literal(*op, *f, types[*f], *literal),
        // The planner puts a literal on the right of a field and compares no
        // two fields yet.
        Step::Compare { .. } => unreachable!("the planner holds the field on the left"),
    }
}

/// Binds a comparison of field `f`, of type `t`, with `literal`.
fn bind_literal(op: Comparison, f: usize, t: FieldType, literal: Literal) -> BoundStep {
    match (t, literal) {
        // A field without a value on any row makes the comparison unknown on
        // every row (ADR-0005).
        (FieldType::Absent, _) => BoundStep::Constant(Constant::Unknown),
        (FieldType::BoolBytes | FieldType::BoolBits | FieldType::Text | FieldType::List, _) => {
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
            Kind::Booleans(Booleans::Bytes(_), _) => Self::BoolBytes,
            Kind::Booleans(Booleans::Bits(_), _) => Self::BoolBits,
            Kind::Text(..) => Self::Text,
            Kind::List(..) => Self::List,
        }
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
