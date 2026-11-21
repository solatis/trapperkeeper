//! Tests for the JSON form of a rule and for `validate`, which returns the
//! first error it finds. Most are property tests over generated rules and
//! texts (ADR-0018). The exact edges of the limits are tested on trees built
//! by hand.

mod common;

use std::collections::BTreeMap;

use proptest::prelude::*;
use proptest::sample::select;
use serde_json::Value;
use trapperkeeper_engine::expr::{Expr, Segment};
use trapperkeeper_engine::limits::{MAX_DEPTH, MAX_SCOPE_TAGS, MAX_TAG_LENGTH, MAX_TEXT_LENGTH};
use trapperkeeper_engine::{Action, Error, Reason, Rule, parse, validate};
use uuid::Uuid;

use common::{MEMBERSHIP, NUMERIC, PATTERNS, around, rule, valid_expr};

/// One way to break a valid rule.
#[derive(Debug, Clone, Copy)]
enum Defect {
    SampleRateZero,
    SampleRateAboveOne,
    SampleRateNan,
    NilRuleId,
    RandomRuleId,
    TooManyTags,
    TagKeyEmpty,
    TagKeyTooLong,
    TagValueTooLong,
    TextTooLong,
}

/// Every variant of `Defect`. The two defect properties draw their defects from
/// this array alone. The compiler checks the match in `apply` for completeness
/// and does not check this array, so a variant that is missing here is never
/// tested.
const DEFECTS: [Defect; 10] = [
    Defect::SampleRateZero,
    Defect::SampleRateAboveOne,
    Defect::SampleRateNan,
    Defect::NilRuleId,
    Defect::RandomRuleId,
    Defect::TooManyTags,
    Defect::TagKeyEmpty,
    Defect::TagKeyTooLong,
    Defect::TagValueTooLong,
    Defect::TextTooLong,
];

/// The scope with one tag.
fn one_tag(key: &str, value: &str) -> BTreeMap<String, String> {
    BTreeMap::from([(key.to_owned(), value.to_owned())])
}

/// Breaks `rule` with the defect `d`. Returns the JSON pointer and the reason
/// that `validate` must then give. A scope defect replaces the whole scope:
/// the generated scope may already be full, and one more tag would get
/// "too many tags" before the error under test.
fn apply(d: Defect, rule: &mut Rule) -> (String, Reason) {
    let too_long = MAX_TAG_LENGTH + 1;

    match d {
        Defect::SampleRateZero | Defect::SampleRateAboveOne | Defect::SampleRateNan => {
            let sample_rate = match d {
                Defect::SampleRateZero => 0.0,
                // `1.0 + f64::EPSILON` is the smallest f64 above 1, so the
                // rate sits directly past the inclusive upper bound.
                Defect::SampleRateAboveOne => 1.0 + f64::EPSILON,
                // The outer arm admits three defects, so `_` is
                // `SampleRateNan` here. Naming it would make this match
                // non-exhaustive, because the compiler does not know that
                // the outer arm admits only three variants.
                _ => f64::NAN,
            };
            rule.sample_rate = sample_rate;
            (
                "/sample_rate".to_owned(),
                Reason::SampleRateOutOfRange { sample_rate },
            )
        }
        Defect::NilRuleId => {
            rule.rule_id = Uuid::nil();
            // `get_version_num` reports 0 for the nil UUID.
            let version = 0;
            (
                "/rule_id".to_owned(),
                Reason::RuleIdNotTimeOrdered { version },
            )
        }
        Defect::RandomRuleId => {
            // The id is a well-formed version 4 UUID, so the refusal concerns
            // the version alone. `from_random_bytes` sets the version and
            // variant bits over the given bytes, so the fill byte is arbitrary.
            rule.rule_id = uuid::Builder::from_random_bytes([0x5a; 16]).into_uuid();
            let version = 4;
            (
                "/rule_id".to_owned(),
                Reason::RuleIdNotTimeOrdered { version },
            )
        }
        Defect::TooManyTags => {
            let (count, max) = (MAX_SCOPE_TAGS + 1, MAX_SCOPE_TAGS);
            rule.scope = (0..count).map(|i| (format!("k{i}"), "v".into())).collect();
            ("/scope".to_owned(), Reason::TooManyTags { count, max })
        }
        Defect::TagKeyEmpty => {
            rule.scope = one_tag("", "v");
            let (len, max) = (0, MAX_TAG_LENGTH);
            // The empty key is an empty reference token, so the JSON pointer
            // of its tag is `/scope` followed by a slash and nothing after it.
            ("/scope/".to_owned(), Reason::TagKeyLength { len, max })
        }
        Defect::TagKeyTooLong => {
            let k = "k".repeat(too_long);
            rule.scope = one_tag(&k, "v");
            let (len, max) = (too_long, MAX_TAG_LENGTH);
            (format!("/scope/{k}"), Reason::TagKeyLength { len, max })
        }
        Defect::TagValueTooLong => {
            // The key holds `/` and `~`, the two characters that a JSON
            // pointer escapes. RFC 6901 writes `~` as `~0` and `/` as `~1`,
            // which gives the pointer expected below. This defect carries
            // that key, because the defect lies in the value, so the key can
            // hold any characters.
            rule.scope = one_tag("a/b~c", &"v".repeat(too_long));
            let (len, max) = (too_long, MAX_TAG_LENGTH);
            (
                "/scope/a~1b~0c".to_owned(),
                Reason::TagValueLength { len, max },
            )
        }
        Defect::TextTooLong => {
            // The text prints with two quotes around it, so it comes out one
            // character over the cap.
            rule.expression = Expr::Text("x".repeat(MAX_TEXT_LENGTH - 1));
            let (len, max) = (MAX_TEXT_LENGTH + 1, MAX_TEXT_LENGTH);
            // The whole expression is too long, so the error points at it.
            ("/expression".to_owned(), Reason::TextTooLong { len, max })
        }
    }
}

/// Which field `d` breaks, as a number that follows the order in which
/// `validate` checks the fields.
fn field(d: Defect) -> usize {
    match d {
        Defect::NilRuleId | Defect::RandomRuleId => 0,
        Defect::SampleRateZero | Defect::SampleRateAboveOne | Defect::SampleRateNan => 1,
        Defect::TooManyTags
        | Defect::TagKeyEmpty
        | Defect::TagKeyTooLong
        | Defect::TagValueTooLong => 2,
        Defect::TextTooLong => 3,
    }
}

/// The key of the marker field, which a test plants so it can find a place
/// in a tree again. `expr()` never generates a text this long, so no other
/// node looks like the marker.
const MARKER: &str = "the defect stands here";

/// The marker: a field with the one key `MARKER`.
fn marker() -> Expr {
    Expr::Field(vec![Segment::Key(MARKER.to_owned())])
}

/// A pattern that does not compile, because its group is never closed. No
/// strategy generates this text, so a test can find it again in a tree.
const BAD_PATTERN: &str = "(the defect stands here";

/// Generates one of the three pattern functions with `BAD_PATTERN` as its
/// pattern and a random text to search.
fn bad_pattern() -> BoxedStrategy<Expr> {
    (select(PATTERNS.to_vec()), valid_expr())
        .prop_map(|(make, text)| make(BAD_PATTERN.to_owned(), Box::new(text)))
        .boxed()
}

/// Generates a node with the literal `None` in a place where validation
/// refuses it. The other operand, if the kind has one, is the marker.
fn none_operand() -> BoxedStrategy<Expr> {
    let none = || Box::new(Expr::None);
    let marked = || Box::new(marker());
    // The three arms match the cases of `operands_without_none` in
    // `src/validate.rs`. The marker makes the node unlike any generated
    // node, so the test can find it again. `Neg(None)` needs no marker,
    // because `valid_expr()` never builds one.
    prop_oneof![
        (select(NUMERIC.to_vec()), any::<bool>()).prop_map(move |(make, left)| {
            if left {
                make(none(), marked())
            } else {
                make(marked(), none())
            }
        }),
        Just(Expr::Neg(none())),
        select(MEMBERSHIP.to_vec()).prop_map(move |make| make(marked(), none())),
    ]
    .boxed()
}

/// `e` wrapped in `n` layers of `not`.
fn nots(n: usize, e: Expr) -> Expr {
    (0..n).fold(e, |e, _| Expr::Not(Box::new(e)))
}

/// Finds `wanted` inside `document` and returns the JSON pointer to it, or
/// `None` when it is not there.
fn pointer_to(document: &Value, wanted: &Value) -> Option<String> {
    // Comparing before going deeper finds the outermost match.
    if document == wanted {
        return Some(String::new());
    }
    match document {
        Value::Array(items) => items
            .iter()
            .enumerate()
            .find_map(|(i, v)| pointer_to(v, wanted).map(|p| format!("/{i}{p}"))),
        // A key needs no escaping here. The tests only search inside the
        // expression, and its JSON keys never hold a `~` or a `/`. Scope keys
        // can, and the tests never search the scope.
        Value::Object(fields) => fields
            .iter()
            .find_map(|(k, v)| pointer_to(v, wanted).map(|p| format!("/{k}{p}"))),
        _ => None,
    }
}

/// Builds the rule text of a chain with `n` links.
type Chain = fn(usize) -> String;

/// The chains for `a_chain_that_validates_reads_back`. Each is one way a
/// text can make a deep tree. The name shows up when proptest prints a
/// failing input. A sum and a chain of methods do not count toward the
/// parser's nesting cap. Parentheses, `not` and a nested `else` count one
/// level per link.
const CHAINS: [(&str, Chain); 5] = [
    ("sum", |n| vec!["1"; n + 1].join(" + ")),
    ("methods", |n| format!("x{}", ".upper()".repeat(n))),
    ("parentheses", |n| {
        format!("{}x{}", "(".repeat(n), ")".repeat(n))
    }),
    ("not", |n| format!("{}x", "not ".repeat(n))),
    ("else", |n| format!("{}1", "1 if x else ".repeat(n))),
];

/// A rule with the expression `e` and fields that pass validation.
fn rule_with(e: Expr) -> Rule {
    Rule {
        // All zeros still makes a version 7 UUID, which is all `validate`
        // asks of an id.
        rule_id: uuid::Builder::from_unix_timestamp_millis(0, &[0; 10]).into_uuid(),
        rule_title: None,
        action: Action::Observe,
        sample_rate: 1.0,
        scope: BTreeMap::new(),
        expression: e,
    }
}

proptest! {
    // Without a regression file a failure leaves no untracked file in the
    // working tree. A failure prints the minimal failing input.
    #![proptest_config(ProptestConfig {
        failure_persistence: None,
        .. ProptestConfig::default()
    })]

    /// A valid rule writes to JSON, reads back equal, and validates.
    #[test]
    fn a_valid_rule_round_trips_and_validates(rule in rule()) {
        let json = serde_json::to_string(&rule).expect("a rule serializes");
        let back: Rule = serde_json::from_str(&json).expect("the JSON parses");
        // The floats in the rule compare exactly after the round trip.
        // serde_json prints the shortest text that names each f64, and its
        // float_roundtrip feature parses that text back to the same f64.
        prop_assert_eq!(&back, &rule);
        prop_assert_eq!(validate(&rule), Ok(()));
    }

    /// A rule whose JSON holds a property that the model does not have is
    /// refused.
    #[test]
    fn a_rule_with_an_unknown_property_is_refused(rule in rule()) {
        let mut document = serde_json::to_value(&rule).expect("a rule serializes");
        // The test copies the expression under the misspelled key
        // `expresion` and keeps the original, so the parse can fail only on
        // the unknown property. Renaming the key would also fail, but on the
        // missing `expression`, which has no default.
        document["expresion"] = document["expression"].clone();
        prop_assert!(serde_json::from_value::<Rule>(document).is_err());
    }

    /// A rule with one defect gets that defect's reason, at the JSON pointer
    /// of the broken field.
    #[test]
    fn a_defect_is_refused_at_its_place(mut rule in rule(), d in select(DEFECTS.to_vec())) {
        let (at, reason) = apply(d, &mut rule);
        let error = validate(&rule).expect_err("a defective rule is refused");
        // The errors are compared as JSON. One defect puts a NaN in the
        // reason, and a NaN never equals itself. serde_json writes it as null
        // on both sides.
        prop_assert_eq!(
            serde_json::to_value(&error).expect("the error serializes"),
            serde_json::to_value(Error { at, reason }).expect("the error serializes"),
            "{:?}", d
        );
    }

    /// When two fields are broken, the error is about the one that comes
    /// first in the rule's JSON.
    #[test]
    fn the_earlier_field_is_refused_first(
        mut rule in rule(),
        (a, b) in (select(DEFECTS.to_vec()), select(DEFECTS.to_vec())),
    ) {
        // Two defects in the same field would overwrite each other.
        prop_assume!(field(a) != field(b));
        // Each call touches only its own field, so the order of the two
        // calls does not matter.
        let (at_a, reason_a) = apply(a, &mut rule);
        let (at_b, reason_b) = apply(b, &mut rule);
        let (at, reason) = if field(a) < field(b) {
            (at_a, reason_a)
        } else {
            (at_b, reason_b)
        };
        let error = validate(&rule).expect_err("a defective rule is refused");
        // Compared as JSON because of the NaN, as in the property above.
        prop_assert_eq!(
            serde_json::to_value(&error).expect("the error serializes"),
            serde_json::to_value(Error { at, reason }).expect("the error serializes"),
            "{:?} and {:?}", a, b
        );
    }

    /// A tree that is too deep gets `TooDeep`, pointing at the first node
    /// past the cap.
    #[test]
    fn a_node_past_the_depth_cap_is_refused_at_its_place(
        mut rule in rule(),
        (tree, above) in around(nots(MAX_DEPTH, marker())),
    ) {
        // The defect is the marker under `MAX_DEPTH` layers of `not`. On its
        // own, the marker is the only node that is too deep. `around` then
        // puts `above` more nodes on top.
        //
        // 1. We validate the rule with this tree. The error must be
        //    `TooDeep`, because nothing else in the tree is deep or invalid.
        // 2. We work out which node is the first one past the cap. Every
        //    node on top pushes the defect one level down, so that node is
        //    the marker with `above` layers of `not` still around it.
        // 3. We find that node in the rule's JSON and compare its pointer
        //    with the error's. The expected pointer comes from serde's
        //    output, so the test also fails when the table `parts!` in
        //    `src/expr.rs` disagrees with the JSON form.

        // 1. validate the rule with the defect
        rule.expression = tree;
        let error = validate(&rule).expect_err("a tree past the cap is refused");
        prop_assert_eq!(&error.reason, &Reason::TooDeep { max: MAX_DEPTH });

        // 2. work out the first node past the cap
        let past = serde_json::to_value(nots(above, marker())).expect("a tree serializes");

        // 3. compare the pointers
        let document = serde_json::to_value(&rule).expect("a rule serializes");
        let found = pointer_to(&document["expression"], &past).expect("the tree holds the node");
        // `pointer_to` searched inside the expression, so `/expression` goes
        // in front.
        prop_assert_eq!(error.at, format!("/expression{found}"));
    }

    /// A `None` next to an operator that cannot take it gets `NoneOperand`,
    /// pointing at that `None`.
    #[test]
    fn a_none_beside_an_operator_is_refused_at_its_place(
        mut rule in rule(),
        // The strategy also hands back the defect node, so the test can find
        // it in the tree. The count of nodes on top is not needed.
        ((tree, _), node) in none_operand()
            .prop_flat_map(|node| (around(node.clone()), Just(node))),
    ) {
        rule.expression = tree;
        let error = validate(&rule).expect_err("a None beside an operator is refused");
        // Nothing else in the tree is invalid, so the reason is
        // `NoneOperand`.
        prop_assert_eq!(&error.reason, &Reason::NoneOperand);
        // The test finds the defect node in the rule's JSON. The error must
        // point somewhere inside it.
        let document = serde_json::to_value(&rule).expect("a rule serializes");
        let wanted = serde_json::to_value(&node).expect("a tree serializes");
        let found = pointer_to(&document["expression"], &wanted).expect("the tree holds the node");
        let prefix = format!("/expression{found}/");
        prop_assert!(error.at.starts_with(&prefix), "{} under {}", error.at, prefix);
        // The defect node holds exactly one `None`, so if the error points
        // at a `None` inside it, it points at the right one.
        let none = serde_json::to_value(Expr::None).expect("a tree serializes");
        prop_assert_eq!(document.pointer(&error.at), Some(&none));
    }

    /// A pattern that does not compile gets `BadPattern`, pointing at the
    /// pattern.
    #[test]
    fn a_pattern_that_does_not_compile_is_refused_at_its_place(
        mut rule in rule(),
        // `around` buries the defect under up to two random nodes. The test
        // finds the pattern by its text, so it does not need the count.
        (tree, _) in bad_pattern().prop_flat_map(around),
    ) {
        rule.expression = tree;
        let error = validate(&rule).expect_err("a bad pattern is refused");
        // Nothing else in the tree is invalid, so the reason is `BadPattern`.
        // The message is the crate's wording, so the test does not compare
        // it.
        prop_assert!(
            matches!(error.reason, Reason::BadPattern { .. }),
            "{:?}",
            error.reason
        );
        // The test finds the text of `BAD_PATTERN` in the rule's JSON and
        // takes the pointer from there. It never spells out the key of the
        // pattern function or `pattern`, so it also fails when `Expr::key`
        // disagrees with the JSON form.
        let document = serde_json::to_value(&rule).expect("a rule serializes");
        // No strategy generates this text, so the first match is the defect.
        let wanted = Value::String(BAD_PATTERN.to_owned());
        let found = pointer_to(&document["expression"], &wanted).expect("the tree holds the pattern");
        prop_assert_eq!(error.at, format!("/expression{found}"));
    }

    /// A chain that parses and validates can be read back, both from its
    /// JSON and from its printed text.
    #[test]
    fn a_chain_that_validates_reads_back(
        (_, chain) in select(CHAINS.to_vec()),
        // The JSON reader stops at 128 levels. Each `+` takes two of them, an
        // object and an array, so the reader first fails on a sum of 65
        // terms. The range goes past both that and the depth cap.
        n in 1..=80usize,
    ) {
        // Long chains of parentheses, `not` and `else` hit `MAX_NESTING` and
        // do not parse. This test is not about those. A sum and a chain of
        // methods always parse.
        let Ok(tree) = parse(&chain(n)) else {
            return Ok(());
        };
        // Nor is it about chains that fail validation. It checks what a
        // valid rule can rely on.
        if validate(&rule_with(tree.clone())).is_err() {
            return Ok(());
        }
        // The depth cap is what makes this pass. Without it, a sum of 65
        // terms would validate and then fail to read back.
        let json = serde_json::to_string(&tree).expect("a tree serializes");
        let back: Expr = serde_json::from_str(&json).expect("the JSON reads back");
        prop_assert_eq!(&back, &tree);
        // The printed text never nests deeper than the original. `not` and
        // `else` chains print as they were written, and `((x))` is just the
        // field `x`, which prints bare. A valid rule also prints within the
        // length cap, so the printed text parses.
        let printed = parse(&tree.to_string()).expect("the printed text parses");
        prop_assert_eq!(&printed, &tree);
    }
}

/// The reason `validate` gives for a rule with the expression `e`, or `None`
/// when the rule is valid.
fn refusal(e: Expr) -> Option<Reason> {
    validate(&rule_with(e)).err().map(|r| r.reason)
}

/// Parses the rule text `t`.
fn parsed(t: &str) -> Expr {
    parse(t).expect("the text parses")
}

/// `1+1+...+1` with `n` terms.
fn sum(n: usize) -> String {
    vec!["1"; n].join("+")
}

/// The depth cap and the length cap sit exactly where they should, for trees
/// built by hand and for trees parsed from text.
#[test]
fn the_limits_of_the_expression_hold_at_their_edges() {
    let too_deep = Some(Reason::TooDeep { max: MAX_DEPTH });

    // `MAX_DEPTH - 1` layers of `not` plus the literal are exactly
    // `MAX_DEPTH` levels.
    assert_eq!(refusal(nots(MAX_DEPTH - 1, Expr::Bool(true))), None);
    assert_eq!(refusal(nots(MAX_DEPTH, Expr::Bool(true))), too_deep);

    // `+` groups to the left, so a sum of 31 terms is 30 `+` nodes deep
    // plus a leaf. With the `>` on top that is 32 levels.
    assert_eq!(refusal(parsed(&format!("{} > 0", sum(31)))), None);
    // A sum does not count toward the parser's nesting cap, so the parser
    // reads this one. Only validation refuses it.
    assert_eq!(refusal(parsed(&format!("{} > 0", sum(32)))), too_deep);

    // The same 32 terms split into two groups are only 18 levels deep: the
    // `>`, the `+` between the groups, and one group of 16 terms.
    let grouped = format!("({}) + ({}) > 0", sum(16), sum(16));
    assert_eq!(refusal(parsed(&grouped)), None);

    // A text prints with two quotes, so it is two characters longer than
    // its content.
    let max = MAX_TEXT_LENGTH;
    let text = |n| Expr::Text("x".repeat(n));
    assert_eq!(refusal(text(max - 2)), None);
    let len = max + 1;
    assert_eq!(
        refusal(text(max - 1)),
        Some(Reason::TextTooLong { len, max })
    );

    // This text is 3006 characters, so it parses. The printer adds a space
    // after each of the 1499 commas, which makes 4505 characters, and that
    // is over the cap.
    let list = format!("x in ({})", vec!["1"; 1500].join(","));
    let tree = parsed(&list);
    let len = tree.to_string().chars().count();
    assert_eq!((list.chars().count(), len), (3006, 4505));
    assert_eq!(refusal(tree), Some(Reason::TextTooLong { len, max }));
}
