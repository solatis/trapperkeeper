//! Strategies for the engine's tests: expression trees, and rules that hold
//! them.

// Each test crate compiles this module and uses only part of it.
#![allow(dead_code)]

use std::collections::BTreeMap;

use proptest::collection::{btree_map, vec};
use proptest::prelude::*;
use proptest::sample::select;
use trapperkeeper_engine::expr::{Expr, Segment, TypeName};
use trapperkeeper_engine::limits::{MAX_SCOPE_TAGS, MAX_TAG_LENGTH};
use trapperkeeper_engine::{Action, Rule};
use uuid::Uuid;

/// The builder of a node with one operand.
type Unary = fn(Box<Expr>) -> Expr;
/// The builder of a node with two operands.
type Binary = fn(Box<Expr>, Box<Expr>) -> Expr;
/// The builder of a quantifier from its name, its list and its test.
type Quantifier = fn(String, Box<Expr>, Box<Expr>) -> Expr;

/// The kinds of node that hold one operand.
const UNARY: [Unary; 18] = [
    Expr::Not,
    Expr::IsNone,
    Expr::IsNotNone,
    Expr::Neg,
    Expr::Len,
    Expr::IsNan,
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

/// The kinds of node that hold two operands.
const BINARY: [Binary; 15] = [
    Expr::Eq,
    Expr::Ne,
    Expr::Lt,
    Expr::Le,
    Expr::Gt,
    Expr::Ge,
    Expr::In,
    Expr::NotIn,
    Expr::Add,
    Expr::Sub,
    Expr::Mul,
    Expr::Div,
    Expr::Mod,
    Expr::StartsWith,
    Expr::EndsWith,
];

/// The kinds of node that hold a list of operands.
const LISTS: [fn(Vec<Expr>) -> Expr; 3] = [Expr::List, Expr::And, Expr::Or];

/// The two quantifiers.
const QUANTIFIERS: [Quantifier; 2] = [
    |name, list, test| Expr::Any { name, list, test },
    |name, list, test| Expr::All { name, list, test },
];

/// The three pattern functions.
const PATTERNS: [fn(String, Box<Expr>) -> Expr; 3] = [
    |pattern, text| Expr::ReSearch { pattern, text },
    |pattern, text| Expr::ReMatch { pattern, text },
    |pattern, text| Expr::ReFullmatch { pattern, text },
];

const TYPES: [TypeName; 5] = [
    TypeName::Int,
    TypeName::Float,
    TypeName::Str,
    TypeName::Bool,
    TypeName::List,
];

/// Any tree the type can hold, of at most three levels, with finite floats.
pub fn expr() -> BoxedStrategy<Expr> {
    // Lists of at most three and texts of at most eight characters, so a
    // failing case shrinks to something a person can read.
    let text = || ".{0,8}";
    let path = || {
        let segment = prop_oneof![
            text().prop_map(Segment::Key),
            any::<u32>().prop_map(Segment::Index),
        ];
        vec(segment, 0..=3)
    };
    // Finite values only, because a NaN is not equal to itself and JSON has
    // no form for it.
    let float = {
        use proptest::num::f64::{NEGATIVE, NORMAL, POSITIVE, SUBNORMAL, ZERO};
        NEGATIVE | NORMAL | POSITIVE | SUBNORMAL | ZERO
    };
    let leaf = prop_oneof![
        Just(Expr::None),
        any::<bool>().prop_map(Expr::Bool),
        any::<i64>().prop_map(Expr::Int),
        float.prop_map(Expr::Float),
        text().prop_map(Expr::Text),
        path().prop_map(Expr::Field),
        (text(), path()).prop_map(|(name, path)| Expr::Element { name, path }),
    ];
    // Every kind of node is generated, and nothing else is constrained. An
    // `And` may hold an `And`, and an `Element` may stand anywhere.
    leaf.prop_recursive(2, 24, 3, move |inner| {
        let one = || inner.clone().prop_map(Box::new);
        prop_oneof![
            (select(UNARY.to_vec()), one()).prop_map(|(make, a)| make(a)),
            (select(BINARY.to_vec()), one(), one()).prop_map(|(make, a, b)| make(a, b)),
            (select(LISTS.to_vec()), vec(inner.clone(), 0..=3))
                .prop_map(|(make, items)| make(items)),
            (one(), one(), one()).prop_map(|(test, then, otherwise)| Expr::If {
                test,
                then,
                otherwise
            }),
            (select(QUANTIFIERS.to_vec()), text(), one(), one())
                .prop_map(|(make, name, list, test)| make(name, list, test)),
            (one(), vec(select(TYPES.to_vec()), 0..=3))
                .prop_map(|(value, types)| Expr::IsInstance { value, types }),
            (select(PATTERNS.to_vec()), text(), one())
                .prop_map(|(make, pattern, text)| make(pattern, text)),
        ]
    })
    .boxed()
}

/// A version 7 id from a generated timestamp and generated random bytes.
pub fn rule_id() -> impl Strategy<Value = Uuid> {
    // A version 7 id holds 48 bits of milliseconds.
    (0..1u64 << 48, any::<[u8; 10]>())
        .prop_map(|(millis, bytes)| uuid::Builder::from_unix_timestamp_millis(millis, &bytes))
        .prop_map(uuid::Builder::into_uuid)
}

/// A scope within the caps: 0 to `MAX_SCOPE_TAGS` tags, each key and value
/// of 1 to `MAX_TAG_LENGTH` characters.
pub fn scope() -> impl Strategy<Value = BTreeMap<String, String>> {
    let text = || {
        proptest::string::string_regex(&format!(".{{1,{MAX_TAG_LENGTH}}}"))
            .expect("a valid pattern")
    };
    btree_map(text(), text(), 0..=MAX_SCOPE_TAGS)
}

/// A rule that `validate` accepts: a version 7 id, a sample rate in (0, 1],
/// a scope within the caps and any expression.
pub fn rule() -> impl Strategy<Value = Rule> {
    let title = proptest::option::of(".{0,40}");
    let action = select(vec![Action::Observe, Action::Drop, Action::Throw]);
    let sample_rate = prop_oneof![Just(1.0), f64::MIN_POSITIVE..=1.0];
    (rule_id(), title, action, sample_rate, scope(), expr()).prop_map(
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
