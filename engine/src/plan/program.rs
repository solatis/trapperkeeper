//! The planner. It turns the expression trees of a rule set into a
//! `Program`: one flat list of simple steps that the executor runs from first
//! to last.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};

use uuid::Uuid;

use super::fields::FieldTree;
use crate::exec::Fault;
use crate::expr::Expr;
use crate::rule::Rule;

/// A rule set turned into a list of steps that the executor runs in order.
///
/// Each step computes one value per row, such as the result of `x > 5`, and
/// stores it under its own index in the list. That index is the step's
/// *slot*. A later step reads the result of an earlier one by its slot, so
/// `x > 5 and y` becomes three steps: slot 0 holds `x > 5`, slot 1 holds
/// `y`, and slot 2 holds `and(0, 1)`. The result of a rule is the slot of the
/// last step of its expression.
#[derive(Debug, Clone)]
pub struct Program {
    fields: FieldTree,
    steps: Vec<Step>,
    // For each rule, in the order of the rule set: its id and the slot that
    // holds its result.
    rules: Vec<(Uuid, usize)>,
}

/// One step of a program. A step refers to a field by the field's number
/// (see `FieldTree`) and to an earlier step by that step's slot.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Step {
    // The same result on every row, such as the literal `True`, or `1 < 2`
    // after the planner computed it.
    Constant(Constant),
    // `field op literal` or `field op field`, such as `x > 5` or `x < y`.
    // The planner always puts a field on the left, so `5 < x` is stored as
    // `x > 5`.
    Compare {
        op: Comparison,
        left: Operand,
        right: Operand,
    },
    // A field used on its own as a condition, as `x` is in `x and y`. It is
    // the field's boolean value, and a fault on a row whose value is not a
    // boolean.
    Condition(usize),
    // `x is None` for the field with this number: true on the rows where the
    // field has no value, false elsewhere, and never unknown (ADR-0005).
    // `x is not None` is a `Not` over this step.
    IsNull(usize),
    // `not`, `and` and `or` over the results of earlier steps, by their
    // slots. They follow the three-valued logic of ADR-0005, where a row is
    // true, false or unknown. `And` and `Or` keep the operands in the order
    // the rule wrote them, because that order decides which fault counts:
    // in `a and b`, a fault of `b` is ignored on rows where `a` is false.
    Not(usize),
    And(usize, usize),
    Or(usize, usize),
}

/// One side of a comparison: a field, by its number, or a literal.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Operand {
    Field(usize),
    Literal(Literal),
}

/// A constant value in a comparison, such as the `5` in `x > 5`.
///
/// `None` is not a literal here. The planner turns `x == None` into a null
/// test (`Step::IsNull`) before it builds a comparison. Text and list
/// literals are not compared yet, so `plan` refuses a rule that compares one.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Literal {
    Int(i64),
    Float(f64),
    Bool(bool),
}

// The planner shares a step between rules when the steps are equal, and it
// finds equal steps through a `HashMap`, which needs `Eq` and `Hash`. `f64`
// has neither, because NaN != NaN. So two float literals count as equal
// when their bits are equal. The only cost is that `x > 0.0` and `x > -0.0`
// are two steps.
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

/// The comparison operator: `==`, `!=`, `<`, `<=`, `>` or `>=`.
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
    /// The operator to use when the two sides swap places: `5 < x` means the
    /// same as `x > 5`, so `Lt` flips to `Gt`.
    fn flip(self) -> Self {
        match self {
            Self::Lt => Self::Gt,
            Self::Le => Self::Ge,
            Self::Gt => Self::Lt,
            Self::Ge => Self::Le,
            Self::Eq | Self::Ne => self,
        }
    }

    /// Whether the operator is `<`, `<=`, `>` or `>=`. Only numbers can be
    /// compared with these.
    pub(crate) fn is_ordering(self) -> bool {
        !matches!(self, Self::Eq | Self::Ne)
    }

    /// Whether `a op b` is true, given how `a` and `b` compare.
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

/// A result that is the same on every row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Constant {
    True,
    False,
    Unknown,
    Fault(Fault),
}

impl Constant {
    /// `True` or `False`.
    pub(crate) fn of(b: bool) -> Self {
        if b { Self::True } else { Self::False }
    }
}

/// Turns `rules` into one program. Fails on the first rule that uses an
/// expression this version cannot run yet, such as arithmetic.
///
/// # Errors
///
/// `Unsupported` with the rule's id and the JSON key of the kind.
pub fn plan(rules: &[Rule]) -> Result<Program, Unsupported> {
    // Each rule's expression is a tree, as the parser built it. Planning
    // flattens each tree into the one list of steps, children before their
    // parent, so a step only reads slots that come before it and the
    // executor can run the list from front to back. The tree already settled
    // precedence, so `a or b and c` arrives as `or(a, and(b, c))`.
    //
    // 1. Number the fields of all rules first, so that a field's number does
    //    not depend on the order in which the rules are planned.
    // 2. Flatten the rules one after another into the same list, so a later
    //    rule can reuse a step that an earlier rule already added.

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
            // The planner returns only the JSON key of the expression it
            // refused, so the rule's id is added here.
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

/// The state of `plan` while it flattens the rules.
struct Planner<'f> {
    fields: &'f FieldTree,
    // The steps added so far, each at the index of its slot.
    steps: Vec<Step>,
    // The slot of each step in `steps`, to find an equal step quickly.
    seen: HashMap<Step, usize>,
}

impl Planner<'_> {
    /// Adds the steps that compute `e` as a condition, true, false or unknown
    /// per row, and returns the slot of the result. Fails with the JSON key
    /// of the first expression in `e` that this version cannot run.
    fn expr(&mut self, e: &Expr) -> Result<usize, &'static str> {
        // One match arm per kind of expression. The arms for `== None` and
        // `!= None` must come before the plain `Eq` and `Ne` arms, because
        // `x == None` is a null test and not a comparison of values
        // (ADR-0005).
        let step = match e {
            Expr::Bool(b) => Step::Constant(Constant::of(*b)),
            Expr::Field(path) => Step::Condition(self.fields.field_of(path)),
            // `None` used as a condition is unknown (ADR-0005).
            Expr::None => Step::Constant(Constant::Unknown),
            // A number, text or list used as a condition, as in `5 and x`, is
            // a fault on every record, because the language does not treat
            // non-booleans as true or false (ADR-0041).
            Expr::Int(_) | Expr::Float(_) | Expr::Text(_) | Expr::List(_) => {
                Step::Constant(Constant::Fault(Fault::NotBoolean))
            }
            // The `?` adds the steps of `x` first, so `Not` reads a slot that
            // is already in the list.
            Expr::Not(x) => Step::Not(self.expr(x)?),
            Expr::And(items) => return self.chain(items, Constant::True, Step::And),
            Expr::Or(items) => return self.chain(items, Constant::False, Step::Or),
            Expr::IsNone(x) => return self.null_test(x),
            Expr::IsNotNone(x) => Step::Not(self.null_test(x)?),
            // `x == None` means `x is None`, and `x != None` means
            // `x is not None`. `None` may stand on either side.
            Expr::Eq(l, r) if **r == Expr::None => return self.null_test(l),
            Expr::Eq(l, r) if **l == Expr::None => return self.null_test(r),
            Expr::Ne(l, r) if **r == Expr::None => Step::Not(self.null_test(l)?),
            Expr::Ne(l, r) if **l == Expr::None => Step::Not(self.null_test(r)?),
            Expr::Eq(l, r) => return self.compare(Comparison::Eq, l, r),
            Expr::Ne(l, r) => return self.compare(Comparison::Ne, l, r),
            Expr::Lt(l, r) => return self.compare(Comparison::Lt, l, r),
            Expr::Le(l, r) => return self.compare(Comparison::Le, l, r),
            Expr::Gt(l, r) => return self.compare(Comparison::Gt, l, r),
            Expr::Ge(l, r) => return self.compare(Comparison::Ge, l, r),
            // Arithmetic, quantifiers and text methods are not supported
            // yet.
            _ => return Err(e.key()),
        };
        Ok(self.push(step))
    }

    /// Adds the steps of an `and` or an `or` over `items`, where `join`
    /// builds the `And` or `Or` step, and returns the slot of the result.
    /// With no items, the result is `empty` on every row.
    fn chain(
        &mut self,
        items: &[Expr],
        empty: Constant,
        join: fn(usize, usize) -> Step,
    ) -> Result<usize, &'static str> {
        // The JSON form allows an `and` or an `or` with no operands. It gives
        // what Python's `all([])` and `any([])` give: true and false.
        let Some((first, rest)) = items.split_first() else {
            return Ok(self.push(Step::Constant(empty)));
        };
        // `a and b and c` becomes `(a and b) and c`, so each `And` step has
        // the earlier operands on its left, in the order the rule wrote them
        // (ADR-0005).
        let mut slot = self.expr(first)?;
        for item in rest {
            let right = self.expr(item)?;
            slot = self.push(join(slot, right));
        }
        Ok(slot)
    }

    /// Adds the step for `e is None` and returns its slot. Fails with the
    /// JSON key of `e` when this version cannot test `e` for a null, such as
    /// when `e` is arithmetic.
    fn null_test(&mut self, e: &Expr) -> Result<usize, &'static str> {
        let step = match e {
            // A field is tested per row, in the executor.
            Expr::Field(path) => Step::IsNull(self.fields.field_of(path)),
            // `None is None` is true.
            Expr::None => Step::Constant(Constant::True),
            // Any other literal, such as `5 is None`, is false, because only
            // `None` is a null.
            Expr::Bool(_) | Expr::Int(_) | Expr::Float(_) | Expr::Text(_) | Expr::List(_) => {
                Step::Constant(Constant::False)
            }
            // Arithmetic and the other kinds that the planner does not
            // support yet.
            _ => return Err(e.key()),
        };
        Ok(self.push(step))
    }

    /// Adds the step for `left op right` and returns its slot.
    fn compare(
        &mut self,
        op: Comparison,
        left: &Expr,
        right: &Expr,
    ) -> Result<usize, &'static str> {
        let step = match (self.operand(left)?, self.operand(right)?) {
            // `1 < 2` is the same on every row, so the planner computes it
            // now and stores the result as a constant.
            (Operand::Literal(l), Operand::Literal(r)) => Step::Constant(fold(op, l, r)),
            // `5 < x` becomes `x > 5`, because the executor's loops expect
            // the field on the left.
            (literal @ Operand::Literal(_), field @ Operand::Field(_)) => Step::Compare {
                op: op.flip(),
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

    /// Converts one side of a comparison: a field becomes its number, and an
    /// int, float or boolean becomes a `Literal`. Fails with the JSON key of
    /// `e` for anything else, such as a text literal or arithmetic.
    fn operand(&self, e: &Expr) -> Result<Operand, &'static str> {
        match e {
            Expr::Field(path) => Ok(Operand::Field(self.fields.field_of(path))),
            Expr::Int(i) => Ok(Operand::Literal(Literal::Int(*i))),
            Expr::Float(x) => Ok(Operand::Literal(Literal::Float(*x))),
            Expr::Bool(b) => Ok(Operand::Literal(Literal::Bool(*b))),
            // Text, lists, arithmetic and function calls are not compared
            // yet. `None < x` would land here too, but validation already
            // refuses it before the rule reaches the planner.
            _ => Err(e.key()),
        }
    }

    /// Adds `step` to the list and returns its slot. When an equal step is
    /// already in the list, it returns that step's slot instead, so two
    /// rules that both test `x > 5` share one step and the executor computes
    /// `x > 5` once.
    fn push(&mut self, step: Step) -> usize {
        if let Some(&slot) = self.seen.get(&step) {
            return slot;
        }
        self.steps.push(step.clone());
        self.seen.insert(step, self.steps.len() - 1);
        self.steps.len() - 1
    }
}

/// Computes `left op right` for two literals, such as `1 < 2.5`.
fn fold(op: Comparison, left: Literal, right: Literal) -> Constant {
    // The arms for booleans come first, because the last arm converts
    // whatever reaches it to `f64`, and a boolean must not reach it.
    let order = match (left, right) {
        // `True < False` and `True < 1` are a fault, because only numbers can
        // be ordered (ADR-0041).
        (Literal::Bool(_), _) | (_, Literal::Bool(_)) if op.is_ordering() => {
            return Constant::Fault(Fault::Unordered);
        }
        (Literal::Bool(l), Literal::Bool(r)) => l.cmp(&r),
        // `True == 1` is false and `True != 1` true, because a boolean never
        // equals a number (ADR-0041).
        (Literal::Bool(_), _) | (_, Literal::Bool(_)) => {
            return Constant::of(op == Comparison::Ne);
        }
        (Literal::Int(l), Literal::Int(r)) => l.cmp(&r),
        // At least one side is a float, so both compare as `f64`, as in
        // `1 < 2.5`. `partial_cmp` follows IEEE 754, so `-0.0 == 0.0` holds.
        // It fails only for a NaN, and a literal is never NaN or infinite,
        // because JSON cannot write either.
        (l, r) => as_f64(l)
            .partial_cmp(&as_f64(r))
            .expect("a float literal is finite"),
    };
    Constant::of(op.holds(order))
}

/// Converts an int or float literal to `f64`.
fn as_f64(l: Literal) -> f64 {
    match l {
        // An int above 2^53 rounds to the nearest `f64`, which the language
        // accepts for a comparison of an int with a float.
        #[expect(
            clippy::cast_precision_loss,
            reason = "an int widens to a float (ADR-0041)"
        )]
        Literal::Int(i) => i as f64,
        Literal::Float(x) => x,
        Literal::Bool(_) => unreachable!("`fold` handles a boolean before it widens"),
    }
}

/// The error of `plan`: a rule uses an expression that this version cannot
/// run yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unsupported {
    pub rule_id: Uuid,
    /// The JSON key of the expression that the planner refused, such as
    /// `add`.
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
    /// The fields that the rules read. The caller must find a column for
    /// each of them and pass the columns to `bind` and `run` in the order of
    /// `FieldTree::paths`.
    #[must_use]
    pub fn fields(&self) -> &FieldTree {
        &self.fields
    }

    /// The steps, each at the index of its slot.
    pub(crate) fn steps(&self) -> &[Step] {
        &self.steps
    }

    /// For each rule: its id and the slot that holds its result.
    pub(crate) fn rules(&self) -> &[(Uuid, usize)] {
        &self.rules
    }
}
