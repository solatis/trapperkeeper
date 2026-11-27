//! The program of a rule set: its steps, and the planner that makes them.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};

use uuid::Uuid;

use super::fields::FieldTree;
use crate::exec::Fault;
use crate::expr::Expr;
use crate::rule::Rule;

/// A rule set planned into one list of steps. Each step writes the slot with
/// its own number, and the result of each rule is the slot of its expression.
#[derive(Debug, Clone)]
pub struct Program {
    fields: FieldTree,
    steps: Vec<Step>,
    // The id of each rule with the slot that holds its result, in the order
    // of the rule set.
    rules: Vec<(Uuid, usize)>,
}

/// One step of a program. A step reads fields by their numbers and the
/// results of earlier steps by their slots, and a comparison may hold a
/// literal.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Step {
    // A result that no field decides, the same on every row.
    Constant(Constant),
    // One of the six comparisons of a field with a literal or with another
    // field. A field is always on the left.
    Compare {
        op: Comparison,
        left: Operand,
        right: Operand,
    },
    // A field where a condition is expected. It faults on a row whose value
    // is not a boolean.
    Condition(usize),
    // `is None`, or `is not None` when `negated`, which is never unknown
    // (ADR-0005).
    IsNull {
        field: usize,
        negated: bool,
    },
    // The logic of ADR-0005 over the results of earlier steps. `And` and `Or`
    // hold their operands in written order, because a fault of the right
    // operand does not count where the left one decided the result.
    Not(usize),
    And(usize, usize),
    Or(usize, usize),
}

/// What a comparison compares: a field by its number, or a literal.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Operand {
    Field(usize),
    Literal(Literal),
}

/// A literal that a comparison holds.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Literal {
    Int(i64),
    Float(f64),
    Bool(bool),
}

// Two literals are one step when their bits are equal, so `0.0` and `-0.0`
// are two steps. Equality of the bits is reflexive, which `Eq` asks for and
// `==` on an `f64` is not for a NaN.
impl PartialEq for Literal {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => a.to_bits() == b.to_bits(),
            (Self::Bool(a), Self::Bool(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for Literal {}

impl Hash for Literal {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::Int(i) => (0u8, *i).hash(state),
            Self::Float(x) => (1u8, x.to_bits()).hash(state),
            Self::Bool(b) => (2u8, *b).hash(state),
        }
    }
}

/// The six comparisons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Comparison {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl Comparison {
    /// The comparison with its operands swapped, so `5 < x` becomes `x > 5`.
    fn mirrored(self) -> Self {
        match self {
            Self::Lt => Self::Gt,
            Self::Le => Self::Ge,
            Self::Gt => Self::Lt,
            Self::Ge => Self::Le,
            Self::Eq | Self::Ne => self,
        }
    }

    /// Whether the comparison orders its operands, which only numbers allow.
    pub(crate) fn is_ordering(self) -> bool {
        !matches!(self, Self::Eq | Self::Ne)
    }

    /// Whether the comparison holds for two values whose order is `order`.
    pub(crate) fn holds(self, order: Ordering) -> bool {
        match self {
            Self::Eq => order.is_eq(),
            Self::Ne => order.is_ne(),
            Self::Lt => order.is_lt(),
            Self::Le => order.is_le(),
            Self::Gt => order.is_gt(),
            Self::Ge => order.is_ge(),
        }
    }
}

/// A result that every row of a chunk, the rows that the executor runs at
/// once, shares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Constant {
    True,
    False,
    Unknown,
    Fault(Fault),
}

impl Constant {
    pub(crate) fn of(b: bool) -> Self {
        if b { Self::True } else { Self::False }
    }
}

/// Plans `rules` into one program. Fails on the first rule that holds a kind
/// of expression this version does not plan.
///
/// # Errors
///
/// `Unsupported` with the rule's id and the JSON key of the kind.
pub fn plan(rules: &[Rule]) -> Result<Program, Unsupported> {
    // The function collects the fields of every rule, then plans the
    // expression of each rule into steps and records the slot of its result.
    //
    // 1. It builds the tree of fields from all the rules first, so that the
    //    number of a field does not depend on which rule the planner reaches
    //    first.
    // 2. It plans the rules one after another into one list of steps, so a
    //    later rule reuses a step that an earlier rule planned.

    // 1. build the tree of fields
    let fields = FieldTree::of(rules.iter().map(|r| &r.expression));

    // 2. plan each rule
    let mut planner = Planner {
        fields: &fields,
        steps: Vec::new(),
        seen: HashMap::new(),
    };
    let rules = rules
        .iter()
        .map(|r| match planner.expr(&r.expression) {
            Ok(slot) => Ok((r.rule_id, slot)),
            // The planner reports the kind, and only this loop knows the
            // rule's id.
            Err(key) => Err(Unsupported {
                rule_id: r.rule_id,
                key,
            }),
        })
        .collect::<Result<_, _>>()?;
    let steps = planner.steps;
    Ok(Program {
        fields,
        steps,
        rules,
    })
}

/// The state of `plan` while it plans: the tree of fields, the steps so far,
/// and the slot of each step that it has planned.
struct Planner<'f> {
    fields: &'f FieldTree,
    steps: Vec<Step>,
    seen: HashMap<Step, usize>,
}

impl Planner<'_> {
    /// Plans `e` where a condition is expected, and returns the slot of its
    /// result. Fails with the JSON key of the first kind of node that it does
    /// not plan.
    fn expr(&mut self, e: &Expr) -> Result<usize, &'static str> {
        // The function tests the kind of `e` in a match, and the first arm
        // that applies plans it. A comparison with the literal `None` comes
        // before the other comparisons, because `x == None` is a test for a
        // null and no comparison (ADR-0005).
        let step = match e {
            Expr::Bool(b) => Step::Constant(Constant::of(*b)),
            Expr::Field(path) => Step::Condition(self.fields.field_of(path)),
            // A null where a condition is expected is unknown (ADR-0005).
            Expr::None => Step::Constant(Constant::Unknown),
            // The language has no truthiness (ADR-0041), so any other literal
            // where a condition is expected is a fault on every record.
            Expr::Int(_) | Expr::Float(_) | Expr::Text(_) | Expr::List(_) => {
                Step::Constant(Constant::Fault(Fault::NotBoolean))
            }
            Expr::Not(x) => Step::Not(self.expr(x)?),
            Expr::And(items) => return self.chain(items, Constant::True, Step::And),
            Expr::Or(items) => return self.chain(items, Constant::False, Step::Or),
            Expr::IsNone(x) => return self.null_test(x, false),
            Expr::IsNotNone(x) => return self.null_test(x, true),
            // `x == None` is `x is None`, and `x != None` is `x is not None`.
            Expr::Eq(l, r) if **r == Expr::None => return self.null_test(l, false),
            Expr::Eq(l, r) if **l == Expr::None => return self.null_test(r, false),
            Expr::Ne(l, r) if **r == Expr::None => return self.null_test(l, true),
            Expr::Ne(l, r) if **l == Expr::None => return self.null_test(r, true),
            Expr::Eq(l, r) => return self.compare(Comparison::Eq, l, r),
            Expr::Ne(l, r) => return self.compare(Comparison::Ne, l, r),
            Expr::Lt(l, r) => return self.compare(Comparison::Lt, l, r),
            Expr::Le(l, r) => return self.compare(Comparison::Le, l, r),
            Expr::Gt(l, r) => return self.compare(Comparison::Gt, l, r),
            Expr::Ge(l, r) => return self.compare(Comparison::Ge, l, r),
            // Any other kind, such as arithmetic, a quantifier or a text
            // method, is not planned yet.
            _ => return Err(e.key()),
        };
        Ok(self.push(step))
    }

    /// Plans the operands of an `and` or an `or` and joins them with `join`,
    /// or returns `empty` for a node without operands.
    fn chain(
        &mut self,
        items: &[Expr],
        empty: Constant,
        join: fn(usize, usize) -> Step,
    ) -> Result<usize, &'static str> {
        // The JSON form reads an `and` or an `or` without operands. It gives
        // what Python's `all` and `any` give for an empty list.
        let Some((first, rest)) = items.split_first() else {
            return Ok(self.push(Step::Constant(empty)));
        };
        // The chain groups from the left, so `a and b and c` reads as
        // `(a and b) and c` and keeps the written order of ADR-0005.
        let mut slot = self.expr(first)?;
        for item in rest {
            let right = self.expr(item)?;
            slot = self.push(join(slot, right));
        }
        Ok(slot)
    }

    /// Plans the test whether `e` is a null, or, when `negated`, whether it is
    /// not. Fails with the JSON key of a kind that this version does not
    /// test.
    fn null_test(&mut self, e: &Expr, negated: bool) -> Result<usize, &'static str> {
        let step = match e {
            Expr::Field(path) => Step::IsNull {
                field: self.fields.field_of(path),
                negated,
            },
            Expr::None => Step::Constant(Constant::of(!negated)),
            // A literal other than `None` is never a null.
            Expr::Bool(_) | Expr::Int(_) | Expr::Float(_) | Expr::Text(_) | Expr::List(_) => {
                Step::Constant(Constant::of(negated))
            }
            _ => return Err(e.key()),
        };
        Ok(self.push(step))
    }

    /// Plans a comparison of `left` with `right`.
    fn compare(
        &mut self,
        op: Comparison,
        left: &Expr,
        right: &Expr,
    ) -> Result<usize, &'static str> {
        let step = match (self.operand(left)?, self.operand(right)?) {
            // No row can change the result of two literals, so the planner
            // compares them itself.
            (Operand::Literal(l), Operand::Literal(r)) => Step::Constant(fold(op, l, r)),
            // The executor's loops hold the field on the left.
            (literal @ Operand::Literal(_), field @ Operand::Field(_)) => Step::Compare {
                op: op.mirrored(),
                left: field,
                right: literal,
            },
            (field @ Operand::Field(_), literal @ Operand::Literal(_)) => Step::Compare {
                op,
                left: field,
                right: literal,
            },
            (left @ Operand::Field(_), right @ Operand::Field(_)) => {
                Step::Compare { op, left, right }
            }
        };
        Ok(self.push(step))
    }

    /// What `e` is as an operand of a comparison. Fails with the JSON key of a
    /// kind that this version does not compare.
    fn operand(&self, e: &Expr) -> Result<Operand, &'static str> {
        match e {
            Expr::Field(path) => Ok(Operand::Field(self.fields.field_of(path))),
            Expr::Int(i) => Ok(Operand::Literal(Literal::Int(*i))),
            Expr::Float(x) => Ok(Operand::Literal(Literal::Float(*x))),
            Expr::Bool(b) => Ok(Operand::Literal(Literal::Bool(*b))),
            // Arithmetic, a function or a text is not compared yet. `None`
            // beside an ordering lands here too, which validation refuses
            // before a rule reaches the planner.
            _ => Err(e.key()),
        }
    }

    /// Appends `step` and returns its slot, or returns the slot of an equal
    /// step that it has planned.
    fn push(&mut self, step: Step) -> usize {
        // An equal step reads the same slots and fields, so it gives the same
        // result, and several rules share it.
        if let Some(&slot) = self.seen.get(&step) {
            return slot;
        }
        self.steps.push(step.clone());
        self.seen.insert(step, self.steps.len() - 1);
        self.steps.len() - 1
    }
}

/// The result of comparing two literals.
fn fold(op: Comparison, left: Literal, right: Literal) -> Constant {
    // The function tests the pair of literals in a chain, and the first case
    // that applies gives the order. The boolean cases come before the number
    // cases, because the last number case widens any pair it gets to floats.
    let order = match (left, right) {
        // An ordering is for numbers only (ADR-0041).
        (Literal::Bool(_), Literal::Bool(_)) if op.is_ordering() => {
            return Constant::Fault(Fault::Unordered);
        }
        (Literal::Bool(l), Literal::Bool(r)) => l.cmp(&r),
        // A boolean and a number have types that do not compare.
        (Literal::Bool(_), _) | (_, Literal::Bool(_)) => {
            return Constant::Fault(Fault::Incompatible);
        }
        (Literal::Int(l), Literal::Int(r)) => l.cmp(&r),
        // An int widens to a float (ADR-0041). `partial_cmp` compares as IEEE
        // 754 does, so `-0.0` equals `0.0`. It gives an order for any two
        // finite floats, and a float literal is finite.
        (l, r) => as_f64(l)
            .partial_cmp(&as_f64(r))
            .expect("a float literal is finite"),
    };
    Constant::of(op.holds(order))
}

/// A number literal as a float.
fn as_f64(l: Literal) -> f64 {
    match l {
        // An int above 2^53 rounds to the nearest float.
        #[expect(
            clippy::cast_precision_loss,
            reason = "an int widens to a float (ADR-0041)"
        )]
        Literal::Int(i) => i as f64,
        Literal::Float(x) => x,
        Literal::Bool(_) => unreachable!("`fold` handles a boolean before it widens"),
    }
}

/// A rule that holds a kind of expression this version does not plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unsupported {
    pub rule_id: Uuid,
    /// The JSON key of the kind that the planner refused, such as `add`.
    pub key: &'static str,
}

impl fmt::Display for Unsupported {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "rule {} holds `{}`, which this version does not evaluate",
            self.rule_id, self.key
        )
    }
}

impl std::error::Error for Unsupported {}

impl Program {
    /// The fields that the rules read, as a tree of paths. A caller asks its
    /// adapter for these fields and hands their columns over in the order of
    /// `FieldTree::paths`.
    #[must_use]
    pub fn fields(&self) -> &FieldTree {
        &self.fields
    }

    /// The steps, each at the index of its slot.
    pub(crate) fn steps(&self) -> &[Step] {
        &self.steps
    }

    /// The id of each rule with the slot of its result.
    pub(crate) fn rules(&self) -> &[(Uuid, usize)] {
        &self.rules
    }
}
