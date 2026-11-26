//! Strategies for the engine's tests: expression trees, and rules that hold
//! them.

// Each test crate compiles this module and uses only part of it.
#![allow(dead_code)]

use std::collections::BTreeMap;

use proptest::collection::{btree_map, vec};
use proptest::prelude::*;
use proptest::sample::{Index, select};
use trapperkeeper_engine::expr::{Expr, Segment, TypeName};
use trapperkeeper_engine::limits::{MAX_SCOPE_TAGS, MAX_TAG_LENGTH};
use trapperkeeper_engine::{Action, Rule};
use uuid::Uuid;

/// The builder of a node with one operand.
type Unary = fn(Box<Expr>) -> Expr;
/// The builder of a node with two operands.
type Binary = fn(Box<Expr>, Box<Expr>) -> Expr;
/// The builder of a quantifier from the name of its element, its list and
/// its test.
type Quantifier = fn(String, Box<Expr>, Box<Expr>) -> Expr;

/// The kinds of node with one operand, except `Neg`. `Neg` is apart because
/// validation does not let it hold the literal `None`.
const UNARY: [Unary; 16] = [
    Expr::Not,
    Expr::IsNone,
    Expr::IsNotNone,
    Expr::Len,
    Expr::Upper,
    Expr::Lower,
    Expr::Strip,
    Expr::LStrip,
    Expr::RStrip,
    Expr::IsDigit,
    Expr::IsAlpha,
    Expr::IsAlnum,
    Expr::IsSpace,
    Expr::IsUpper,
    Expr::IsLower,
    Expr::IsAscii,
];

/// The kinds of node with two operands that may hold the literal `None` on
/// either side.
const BINARY: [Binary; 4] = [Expr::Eq, Expr::Ne, Expr::StartsWith, Expr::EndsWith];

/// The orderings and the arithmetic operators. Validation refuses the
/// literal `None` on either side of them.
pub const NUMERIC: [Binary; 9] = [
    Expr::Lt,
    Expr::Le,
    Expr::Gt,
    Expr::Ge,
    Expr::Add,
    Expr::Sub,
    Expr::Mul,
    Expr::Div,
    Expr::Mod,
];

/// `in` and `not in`. Validation refuses the literal `None` on their right
/// side.
pub const MEMBERSHIP: [Binary; 2] = [Expr::In, Expr::NotIn];

/// The kinds of node that hold a list of operands.
const LISTS: [fn(Vec<Expr>) -> Expr; 3] = [Expr::List, Expr::And, Expr::Or];

/// The two quantifiers.
const QUANTIFIERS: [Quantifier; 2] = [
    |name, list, test| Expr::Any { name, list, test },
    |name, list, test| Expr::All { name, list, test },
];

/// The three pattern functions.
pub const PATTERNS: [fn(String, Box<Expr>) -> Expr; 3] = [
    |pattern, text| Expr::ReSearch { pattern, text },
    |pattern, text| Expr::ReMatch { pattern, text },
    |pattern, text| Expr::ReFullmatch { pattern, text },
];

/// Every variant of `TypeName`, the types that `isinstance` tests for.
const TYPES: [TypeName; 5] = [
    TypeName::Int,
    TypeName::Float,
    TypeName::Str,
    TypeName::Bool,
    TypeName::List,
];

/// The regex for a generated text: up to eight characters. `.` matches no
/// line break, so a text never has one.
const TEXT: &str = ".{0,8}";

/// Generates a pattern that compiles: one of a few realistic patterns, or a
/// short text of letters, digits and spaces.
pub fn pattern() -> BoxedStrategy<String> {
    prop_oneof![
        // An anchor, a class, a counted repeat and an alternation. The list
        // has no counted repeat of a Unicode class, such as `^\w{1,64}$`:
        // that compiles to megabytes and would slow down every test.
        select(vec!["^ab", "[0-9]+$", "x{2,4}", "cat|dog"]).prop_map(str::to_owned),
        // Letters, digits and spaces mean nothing special in a pattern, so
        // any such text compiles.
        "[a-zA-Z0-9 ]{0,8}",
    ]
    .boxed()
}

/// Generates every kind of node, in trees of at most three levels, with
/// finite floats, texts of at most eight characters and lists and paths of at
/// most three items. With `valid` set, the tree passes validation.
fn tree(valid: bool) -> BoxedStrategy<Expr> {
    // The small bounds let a failing case shrink to something a person can
    // read.
    let path = || {
        let segment = prop_oneof![
            TEXT.prop_map(Segment::Key),
            any::<u32>().prop_map(Segment::Index),
        ];
        vec(segment, 0..=3)
    };
    // The flags leave out `INFINITE`, `QUIET_NAN` and `SIGNALING_NAN`, so
    // every generated float is finite. JSON has no form for a NaN or an
    // infinity, and the JSON round trips of `tests/tree.rs` and
    // `tests/model.rs` compare for equality, where a NaN differs from itself.
    let float = {
        use proptest::num::f64::{NEGATIVE, NORMAL, POSITIVE, SUBNORMAL, ZERO};
        NEGATIVE | NORMAL | POSITIVE | SUBNORMAL | ZERO
    };
    let leaf = prop_oneof![
        Just(Expr::None),
        any::<bool>().prop_map(Expr::Bool),
        any::<i64>().prop_map(Expr::Int),
        float.prop_map(Expr::Float),
        TEXT.prop_map(Expr::Text),
        path().prop_map(Expr::Field),
        (TEXT, path()).prop_map(|(name, path)| Expr::Element { name, path }),
    ];
    // A depth of 2 recursions above the leaves gives the three levels. The
    // desired size 24 and the expected branch size 3 are choices that no
    // measurement backs.
    leaf.prop_recursive(2, 24, 3, move |inner| {
        let operand = || inner.clone().prop_map(Box::new);
        // An operand for the places where validation refuses the literal
        // `None`. With `valid` set it is never `None`. Without `valid` it can
        // be anything, so `expr()` still builds `x < None` for the JSON
        // tests.
        let not_none = || {
            inner
                .clone()
                .prop_filter("validation refuses None here", move |e| {
                    !valid || !matches!(e, Expr::None)
                })
                .prop_map(Box::new)
        };
        // The strategy does not hold the trees to the shapes the parser
        // builds. An `And` may hold an `And` or fewer than two operands, and
        // an `Element` may stand outside a quantifier that names it. No test
        // fails when a variant of `Expr` is missing from these arms, so
        // `engine/AGENTS.md` asks that a new kind of node is added here.
        prop_oneof![
            (select(UNARY.to_vec()), operand()).prop_map(|(make, a)| make(a)),
            not_none().prop_map(Expr::Neg),
            (select(BINARY.to_vec()), operand(), operand()).prop_map(|(make, a, b)| make(a, b)),
            (select(NUMERIC.to_vec()), not_none(), not_none()).prop_map(|(make, a, b)| make(a, b)),
            (select(MEMBERSHIP.to_vec()), operand(), not_none())
                .prop_map(|(make, a, b)| make(a, b)),
            (select(LISTS.to_vec()), vec(inner.clone(), 0..=3))
                .prop_map(|(make, items)| make(items)),
            (operand(), operand(), operand()).prop_map(|(test, then, otherwise)| Expr::If {
                test,
                then,
                otherwise
            }),
            (select(QUANTIFIERS.to_vec()), TEXT, operand(), operand())
                .prop_map(|(make, name, list, test)| make(name, list, test)),
            (operand(), vec(select(TYPES.to_vec()), 0..=3))
                .prop_map(|(value, types)| Expr::IsInstance { value, types }),
            // Without `valid` the pattern is any text. The JSON form does not
            // care whether a pattern compiles, and its tests should cover
            // that.
            (
                select(PATTERNS.to_vec()),
                if valid { pattern() } else { TEXT.boxed() },
                operand()
            )
                .prop_map(|(make, pattern, text)| make(pattern, text)),
        ]
    })
    .boxed()
}

/// Generates any tree. A pattern in it is any short text and may not compile.
pub fn expr() -> BoxedStrategy<Expr> {
    tree(false)
}

/// Generates a tree that passes validation.
pub fn valid_expr() -> BoxedStrategy<Expr> {
    tree(true)
}

/// Wraps `inner` in one random node. `inner` becomes one of that node's
/// operands, and `valid_expr()` fills in the rest, so only `inner` can make
/// validation fail.
fn frame(inner: Expr) -> BoxedStrategy<Expr> {
    let operand = || valid_expr().prop_map(Box::new);
    // The second operand of a two-operand node is never the literal `None`.
    // Some kinds would refuse it, and filtering it out for all of them lets
    // one arm build every kind.
    let other = || {
        valid_expr()
            .prop_filter("validation refuses None here", |e| !matches!(e, Expr::None))
            .prop_map(Box::new)
    };
    let held = Just(Box::new(inner));
    // `tree` builds `Neg` in an arm of its own, so it is added to `UNARY`
    // here.
    let unary = [&UNARY[..], &[Expr::Neg]].concat();
    let binary = [&BINARY[..], &NUMERIC[..], &MEMBERSHIP[..]].concat();
    // The arms cover every kind of node that has an operand, from the same
    // lists that `tree` uses. So the tests see a pointer through each kind.
    prop_oneof![
        (select(unary), held.clone()).prop_map(|(make, inner)| make(inner)),
        // `left` says which side `inner` goes on.
        (select(binary), held.clone(), other(), any::<bool>()).prop_map(
            |(make, inner, other, left)| {
                if left {
                    make(inner, other)
                } else {
                    make(other, inner)
                }
            }
        ),
        // `inner` goes in at a random position among up to two other items.
        (
            select(LISTS.to_vec()),
            held.clone(),
            vec(valid_expr(), 0..=2),
            any::<Index>()
        )
            .prop_map(|(make, inner, mut items, at)| {
                items.insert(at.index(items.len() + 1), *inner);
                make(items)
            }),
        // `at` picks where `inner` goes: the test, the first branch or the
        // `else` branch.
        (held.clone(), operand(), operand(), 0..3usize).prop_map(|(inner, a, b, at)| {
            let (test, then, otherwise) = match at {
                0 => (inner, a, b),
                1 => (a, inner, b),
                _ => (a, b, inner),
            };
            Expr::If {
                test,
                then,
                otherwise,
            }
        }),
        // `in_list` says whether `inner` is the list or the test.
        (
            select(QUANTIFIERS.to_vec()),
            TEXT,
            held.clone(),
            operand(),
            any::<bool>()
        )
            .prop_map(|(make, name, inner, other, in_list)| {
                if in_list {
                    make(name, inner, other)
                } else {
                    make(name, other, inner)
                }
            }),
        (held.clone(), vec(select(TYPES.to_vec()), 0..=3))
            .prop_map(|(value, types)| Expr::IsInstance { value, types }),
        (select(PATTERNS.to_vec()), pattern(), held)
            .prop_map(|(make, pattern, text)| make(pattern, text)),
    ]
    .boxed()
}

/// Buries `defect` under zero, one or two random nodes. Returns the tree and
/// how many nodes it put on top.
pub fn around(defect: Expr) -> BoxedStrategy<(Expr, usize)> {
    // Two nodes on top, plus the three levels of the other operands, stay
    // far below `MAX_DEPTH`. So only the defect can make validation fail.
    (0..=2usize)
        .prop_flat_map(move |n| {
            // Each round wraps the tree in one more node, so after `n` rounds
            // the defect sits `n` levels below the top.
            (0..n)
                .fold(Just(defect.clone()).boxed(), |tree, _| {
                    tree.prop_flat_map(frame).boxed()
                })
                .prop_map(move |tree| (tree, n))
        })
        .boxed()
}

/// A version 7 UUID from a generated timestamp and generated random bytes.
pub fn rule_id() -> impl Strategy<Value = Uuid> {
    // A version 7 UUID holds 48 bits of milliseconds.
    (0..1u64 << 48, any::<[u8; 10]>())
        .prop_map(|(millis, bytes)| uuid::Builder::from_unix_timestamp_millis(millis, &bytes))
        .prop_map(uuid::Builder::into_uuid)
}

/// A scope within the caps: 0 to `MAX_SCOPE_TAGS` tags, each key and value
/// of 1 to `MAX_TAG_LENGTH` characters.
pub fn scope() -> impl Strategy<Value = BTreeMap<String, String>> {
    // `.` matches no line break, so no generated tag holds one, although
    // `validate` accepts one.
    let text = || {
        proptest::string::string_regex(&format!(".{{1,{MAX_TAG_LENGTH}}}"))
            .expect("a valid pattern")
    };
    btree_map(text(), text(), 0..=MAX_SCOPE_TAGS)
}

/// A rule that `validate` accepts: a version 7 UUID as its id, a sample rate
/// in (0, 1], a scope within the caps and an expression from `valid_expr()`.
pub fn rule() -> impl Strategy<Value = Rule> {
    // The bound of 40 characters on a title is a choice that no measurement
    // backs.
    let title = proptest::option::of(".{0,40}");
    let action = select(vec![Action::Observe, Action::Drop, Action::Throw]);
    // `prop_oneof` draws `Just(1.0)` in about half of the cases, because a
    // draw from the range alone almost never gives 1.0, and that share is a
    // choice. The range starts at `f64::MIN_POSITIVE`, so 0 is never drawn.
    let sample_rate = prop_oneof![Just(1.0), f64::MIN_POSITIVE..=1.0];
    (rule_id(), title, action, sample_rate, scope(), valid_expr()).prop_map(
        |(rule_id, rule_title, action, sample_rate, scope, expression)| Rule {
            rule_id,
            rule_title,
            action,
            sample_rate,
            scope,
            expression,
        },
    )
}
