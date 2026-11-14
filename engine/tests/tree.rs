//! The JSON form of the tree: one row per kind of node pins its key,
//! generated trees round-trip, and a node that is not of the form is refused.

mod common;

use proptest::prelude::*;
use trapperkeeper_engine::expr::{Expr, Segment, TypeName};

use common::expr;

/// How many kinds of node `Expr` has. It is the number of arms of `row`, so
/// it grows with the enum.
const KINDS: usize = 50;

/// The index of the row of `every_kind` that pins the kind of `e`.
fn row(e: &Expr) -> usize {
    // One arm per variant and no wildcard, so a new kind of node does not
    // compile until it has a number here.
    match e {
        Expr::None => 0,
        Expr::Bool(_) => 1,
        Expr::Int(_) => 2,
        Expr::Float(_) => 3,
        Expr::Text(_) => 4,
        Expr::List(_) => 5,
        Expr::Field(_) => 6,
        Expr::Element { .. } => 7,
        Expr::Not(_) => 8,
        Expr::And(_) => 9,
        Expr::Or(_) => 10,
        Expr::Eq(..) => 11,
        Expr::Ne(..) => 12,
        Expr::Lt(..) => 13,
        Expr::Le(..) => 14,
        Expr::Gt(..) => 15,
        Expr::Ge(..) => 16,
        Expr::In(..) => 17,
        Expr::NotIn(..) => 18,
        Expr::IsNone(_) => 19,
        Expr::IsNotNone(_) => 20,
        Expr::Add(..) => 21,
        Expr::Sub(..) => 22,
        Expr::Mul(..) => 23,
        Expr::Div(..) => 24,
        Expr::Mod(..) => 25,
        Expr::Neg(_) => 26,
        Expr::If { .. } => 27,
        Expr::Any { .. } => 28,
        Expr::All { .. } => 29,
        Expr::Len(_) => 30,
        Expr::IsInstance { .. } => 31,
        Expr::ReSearch { .. } => 32,
        Expr::ReMatch { .. } => 33,
        Expr::ReFullmatch { .. } => 34,
        Expr::IsNan(_) => 35,
        Expr::StartsWith(..) => 36,
        Expr::EndsWith(..) => 37,
        Expr::Upper(_) => 38,
        Expr::Lower(_) => 39,
        Expr::Strip(_) => 40,
        Expr::LStrip(_) => 41,
        Expr::RStrip(_) => 42,
        Expr::IsDigit(_) => 43,
        Expr::IsAlpha(_) => 44,
        Expr::IsAlnum(_) => 45,
        Expr::IsSpace(_) => 46,
        Expr::IsUpper(_) => 47,
        Expr::IsLower(_) => 48,
        Expr::IsAscii(_) => KINDS - 1,
    }
}

/// One node of every kind with its compact JSON, in the order of the
/// variants of `Expr`.
#[expect(clippy::too_many_lines, reason = "one row per kind of node")]
fn every_kind() -> Vec<(Expr, &'static str)> {
    let a = || Box::new(Expr::Int(1));
    let b = || Box::new(Expr::Int(2));
    let c = || Box::new(Expr::Int(3));
    let ab = || vec![Expr::Int(1), Expr::Int(2)];
    let key = |k: &str| Segment::Key(k.to_owned());
    let (name, pattern) = (|| "i".to_owned(), || "^a".to_owned());
    vec![
        (Expr::None, r#""none""#),
        (Expr::Bool(true), r#"{"bool":true}"#),
        (Expr::Int(-7), r#"{"int":-7}"#),
        (Expr::Float(1.5), r#"{"float":1.5}"#),
        (Expr::Text("a".to_owned()), r#"{"text":"a"}"#),
        (Expr::List(ab()), r#"{"list":[{"int":1},{"int":2}]}"#),
        (
            Expr::Field(vec![key("items"), Segment::Index(0), key("price")]),
            r#"{"field":["items",0,"price"]}"#,
        ),
        (
            Expr::Element {
                name: name(),
                path: vec![key("price")],
            },
            r#"{"element":{"name":"i","path":["price"]}}"#,
        ),
        (Expr::Not(a()), r#"{"not":{"int":1}}"#),
        (Expr::And(ab()), r#"{"and":[{"int":1},{"int":2}]}"#),
        (Expr::Or(ab()), r#"{"or":[{"int":1},{"int":2}]}"#),
        (Expr::Eq(a(), b()), r#"{"eq":[{"int":1},{"int":2}]}"#),
        (Expr::Ne(a(), b()), r#"{"ne":[{"int":1},{"int":2}]}"#),
        (Expr::Lt(a(), b()), r#"{"lt":[{"int":1},{"int":2}]}"#),
        (Expr::Le(a(), b()), r#"{"le":[{"int":1},{"int":2}]}"#),
        (Expr::Gt(a(), b()), r#"{"gt":[{"int":1},{"int":2}]}"#),
        (Expr::Ge(a(), b()), r#"{"ge":[{"int":1},{"int":2}]}"#),
        (Expr::In(a(), b()), r#"{"in":[{"int":1},{"int":2}]}"#),
        (Expr::NotIn(a(), b()), r#"{"not_in":[{"int":1},{"int":2}]}"#),
        (Expr::IsNone(a()), r#"{"is_none":{"int":1}}"#),
        (Expr::IsNotNone(a()), r#"{"is_not_none":{"int":1}}"#),
        (Expr::Add(a(), b()), r#"{"add":[{"int":1},{"int":2}]}"#),
        (Expr::Sub(a(), b()), r#"{"sub":[{"int":1},{"int":2}]}"#),
        (Expr::Mul(a(), b()), r#"{"mul":[{"int":1},{"int":2}]}"#),
        (Expr::Div(a(), b()), r#"{"div":[{"int":1},{"int":2}]}"#),
        (Expr::Mod(a(), b()), r#"{"mod":[{"int":1},{"int":2}]}"#),
        (Expr::Neg(a()), r#"{"neg":{"int":1}}"#),
        (
            Expr::If {
                test: a(),
                then: b(),
                otherwise: c(),
            },
            r#"{"if":{"test":{"int":1},"then":{"int":2},"else":{"int":3}}}"#,
        ),
        (
            Expr::Any {
                name: name(),
                list: a(),
                test: b(),
            },
            r#"{"any":{"name":"i","list":{"int":1},"test":{"int":2}}}"#,
        ),
        (
            Expr::All {
                name: name(),
                list: a(),
                test: b(),
            },
            r#"{"all":{"name":"i","list":{"int":1},"test":{"int":2}}}"#,
        ),
        (Expr::Len(a()), r#"{"len":{"int":1}}"#),
        (
            // The row also pins the five type names.
            Expr::IsInstance {
                value: a(),
                types: vec![
                    TypeName::Int,
                    TypeName::Float,
                    TypeName::Str,
                    TypeName::Bool,
                    TypeName::List,
                ],
            },
            r#"{"isinstance":{"value":{"int":1},"types":["int","float","str","bool","list"]}}"#,
        ),
        (
            Expr::ReSearch {
                pattern: pattern(),
                text: a(),
            },
            r#"{"re_search":{"pattern":"^a","text":{"int":1}}}"#,
        ),
        (
            Expr::ReMatch {
                pattern: pattern(),
                text: a(),
            },
            r#"{"re_match":{"pattern":"^a","text":{"int":1}}}"#,
        ),
        (
            Expr::ReFullmatch {
                pattern: pattern(),
                text: a(),
            },
            r#"{"re_fullmatch":{"pattern":"^a","text":{"int":1}}}"#,
        ),
        (Expr::IsNan(a()), r#"{"math_isnan":{"int":1}}"#),
        (
            Expr::StartsWith(a(), b()),
            r#"{"startswith":[{"int":1},{"int":2}]}"#,
        ),
        (
            Expr::EndsWith(a(), b()),
            r#"{"endswith":[{"int":1},{"int":2}]}"#,
        ),
        (Expr::Upper(a()), r#"{"upper":{"int":1}}"#),
        (Expr::Lower(a()), r#"{"lower":{"int":1}}"#),
        (Expr::Strip(a()), r#"{"strip":{"int":1}}"#),
        (Expr::LStrip(a()), r#"{"lstrip":{"int":1}}"#),
        (Expr::RStrip(a()), r#"{"rstrip":{"int":1}}"#),
        (Expr::IsDigit(a()), r#"{"isdigit":{"int":1}}"#),
        (Expr::IsAlpha(a()), r#"{"isalpha":{"int":1}}"#),
        (Expr::IsAlnum(a()), r#"{"isalnum":{"int":1}}"#),
        (Expr::IsSpace(a()), r#"{"isspace":{"int":1}}"#),
        (Expr::IsUpper(a()), r#"{"isupper":{"int":1}}"#),
        (Expr::IsLower(a()), r#"{"islower":{"int":1}}"#),
        (Expr::IsAscii(a()), r#"{"isascii":{"int":1}}"#),
    ]
}

#[test]
fn every_kind_writes_and_reads_its_json() {
    let rows = every_kind();
    // `KINDS` counts the variants of `Expr`, so a table that misses a kind
    // of node fails here.
    assert_eq!(rows.len(), KINDS);
    for (i, (node, json)) in rows.iter().enumerate() {
        assert_eq!(row(node), i, "{json} is not at the place of its kind");
        let written = serde_json::to_string(node).expect("a tree serializes");
        assert_eq!(written, *json);
        let read: Expr = serde_json::from_str(json).expect("the JSON parses");
        assert_eq!(&read, node);
    }
}

proptest! {
    // No regression file, so a failure leaves nothing untracked in the tree.
    // The failing seed is in the output.
    #![proptest_config(ProptestConfig {
        failure_persistence: None,
        .. ProptestConfig::default()
    })]

    /// A generated tree writes to JSON and reads back equal.
    #[test]
    fn a_tree_round_trips_through_json(tree in expr()) {
        let json = serde_json::to_string(&tree).expect("a tree serializes");
        let back: Expr = serde_json::from_str(&json).expect("the JSON parses");
        // The floats compare exactly: serde_json prints the shortest text
        // that names the f64, and its float_roundtrip feature parses that
        // text to the nearest f64.
        prop_assert_eq!(back, tree);
    }
}

#[test]
fn a_node_outside_the_form_is_refused() {
    // A misspelled kind is a parse error and no default.
    let unknown_key = r#"{"gtt":[{"int":1},{"int":2}]}"#;
    // The enum denies unknown properties. Without that attribute serde would
    // skip the fourth property of this `if`.
    let extra_property = r#"{"if":{"test":{"int":1},"then":{"int":2},"else":{"int":3},"extra":4}}"#;
    for document in [unknown_key, extra_property] {
        let parsed = serde_json::from_str::<Expr>(document);
        assert!(parsed.is_err(), "{document} was accepted");
    }
}
