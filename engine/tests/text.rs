//! The rule language as text: two properties, and two tables for what a
//! property cannot say. The valid texts pin spellings that the printer never
//! writes. The faulty texts are the specification of the errors: the place,
//! the reason and the sentence of each.

use std::ops::Range;

use proptest::collection::vec;
use proptest::prelude::*;
use proptest::sample::select;
use trapperkeeper_engine::expr::{Expr, Segment, TypeName};
use trapperkeeper_engine::parse;
use trapperkeeper_engine::text::{Found, Reason};

/// The builder of a node with one operand.
type Unary = fn(Box<Expr>) -> Expr;
/// The builder of a node with two operands.
type Binary = fn(Box<Expr>, Box<Expr>) -> Expr;

/// The kinds of node with one operand that have no rule of the canonical
/// form.
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

/// The kinds of node with two operands that have no rule of the canonical
/// form.
const BINARY: [Binary; 13] = [
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

const TYPES: [TypeName; 5] = [
    TypeName::Int,
    TypeName::Float,
    TypeName::Str,
    TypeName::Bool,
    TypeName::List,
];

/// The words of the language, which no name can be.
const WORDS: [&str; 14] = [
    "and", "or", "not", "in", "is", "if", "else", "for", "None", "True", "False", "record", "re",
    "math",
];

/// A name that an author can write bare: a letter or `_`, then letters,
/// digits or `_`, and no word of the language.
fn name() -> impl Strategy<Value = String> {
    "[a-zA-Z_][a-zA-Z0-9_]{0,7}"
        .prop_filter("a word of the language", |n| !WORDS.contains(&n.as_str()))
}

/// Any text of at most eight characters, line breaks included.
fn text() -> &'static str {
    "(?s).{0,8}"
}

/// A canonical tree of at most `depth` levels above its leaves, in which the
/// names of `scope` are bound elements. A canonical tree is one the parser
/// can build.
fn canonical_in(scope: &[String], depth: u32) -> BoxedStrategy<Expr> {
    let steps = |most: usize| {
        let step = prop_oneof![
            text().prop_map(Segment::Key),
            any::<u32>().prop_map(Segment::Index),
        ];
        vec(step, 0..=most)
    };
    // Finite values only, because a tree holds no other float.
    let float = {
        use proptest::num::f64::{NEGATIVE, NORMAL, POSITIVE, SUBNORMAL, ZERO};
        POSITIVE | NEGATIVE | NORMAL | SUBNORMAL | ZERO
    };
    // A field has one to four segments, and the first is a key, because the
    // record is an object. A key is any text, so the bracket form gets
    // exercised.
    let field = (text(), steps(3)).prop_map(|(first, rest)| {
        let key = Segment::Key(first);
        Expr::Field(std::iter::once(key).chain(rest).collect())
    });
    let mut leaves = vec![
        Just(Expr::None).boxed(),
        any::<bool>().prop_map(Expr::Bool).boxed(),
        any::<i64>().prop_map(Expr::Int).boxed(),
        float.prop_map(Expr::Float).boxed(),
        text().prop_map(Expr::Text).boxed(),
        field.boxed(),
    ];
    // An element is one of the names of `scope`, because an element exists
    // only under the quantifier that names it.
    if !scope.is_empty() {
        let element = (select(scope.to_vec()), steps(2));
        leaves.push(
            element
                .prop_map(|(name, path)| Expr::Element { name, path })
                .boxed(),
        );
    }
    let leaf = proptest::strategy::Union::new(leaves);
    // At depth 0 there are only leaves, so the recursion ends.
    let Some(depth) = depth.checked_sub(1) else {
        return leaf.boxed();
    };

    // Lists and operand lists hold at most three, keys and texts at most
    // eight characters, and a tree at most three levels. That is at most 27
    // leaves, so a printed tree stays far under the caps of the parser.
    let sub = canonical_in(scope, depth);
    let one = || sub.clone().prop_map(Box::new);
    // `and` and `or` take two or three operands, and an operand of the same
    // kind is spliced in, as the parser does.
    let chain = |make: fn(Vec<Expr>) -> Expr| {
        vec(sub.clone(), 2..=3).prop_map(move |operands| {
            let kind = std::mem::discriminant(&make(Vec::new()));
            let spliced = operands.into_iter().flat_map(|e| {
                let same = std::mem::discriminant(&e) == kind;
                match e {
                    Expr::And(inner) | Expr::Or(inner) if same => inner,
                    e => vec![e],
                }
            });
            make(spliced.collect())
        })
    };
    // No operand of `==` or `!=` is the literal `None`, because the parser
    // refuses `x == None`.
    let equality = |make: Binary| {
        let no_none = |(l, r): &(Box<Expr>, Box<Expr>)| {
            !matches!(**l, Expr::None) && !matches!(**r, Expr::None)
        };
        (one(), one())
            .prop_filter("an operand is None", no_none)
            .prop_map(move |(l, r)| make(l, r))
    };
    // The test of a quantifier is generated with its name added to `scope`,
    // and its list without it.
    let quantifier = |all: bool| {
        let (scope, list) = (scope.to_vec(), sub.clone());
        name().prop_flat_map(move |name| {
            let inner: Vec<String> = scope.iter().cloned().chain([name.clone()]).collect();
            let test = canonical_in(&inner, depth);
            (Just(name), list.clone(), test).prop_map(move |(name, list, test)| {
                let (list, test) = (Box::new(list), Box::new(test));
                if all {
                    Expr::All { name, list, test }
                } else {
                    Expr::Any { name, list, test }
                }
            })
        })
    };
    let pattern = |make: fn(String, Box<Expr>) -> Expr| {
        (text(), one()).prop_map(move |(pattern, text)| make(pattern, text))
    };
    prop_oneof![
        leaf,
        vec(sub.clone(), 0..=3).prop_map(Expr::List),
        (select(UNARY.to_vec()), one()).prop_map(|(make, a)| make(a)),
        (select(BINARY.to_vec()), one(), one()).prop_map(|(make, a, b)| make(a, b)),
        chain(Expr::And),
        chain(Expr::Or),
        equality(Expr::Eq),
        equality(Expr::Ne),
        (one(), one(), one()).prop_map(|(test, then, otherwise)| Expr::If {
            test,
            then,
            otherwise
        }),
        quantifier(false),
        quantifier(true),
        (one(), vec(select(TYPES.to_vec()), 1..=3))
            .prop_map(|(value, types)| Expr::IsInstance { value, types }),
        pattern(|pattern, text| Expr::ReSearch { pattern, text }),
        pattern(|pattern, text| Expr::ReMatch { pattern, text }),
        pattern(|pattern, text| Expr::ReFullmatch { pattern, text }),
    ]
    .boxed()
}

/// (text, the text its tree prints as).
const VALID: &[(&str, &str)] = &[
    ("amount > 100", "amount > 100"),
    (
        "key.val1 > key.val2 and key.val3 > 0",
        "key.val1 > key.val2 and key.val3 > 0",
    ),
    (
        "status not in ('paid', 'shipped') or discount is not None",
        "status not in (\"paid\", \"shipped\") or discount is not None",
    ),
    (
        "items[0].lines[2].sku == 'a\\'b\\n'",
        "items[0].lines[2].sku == \"a'b\\n\"",
    ),
    // A lowercase `null` or `true` is a field.
    ("status == null", "status == null"),
    ("active == true", "active == true"),
    // Python's other keywords are fields.
    (
        "from == \"AMS\" and class == 1",
        "from == \"AMS\" and class == 1",
    ),
    // The bracket form, at the record and at a later step.
    ("record[\"unit price\"] > 2", "record[\"unit price\"] > 2"),
    ("flows[\"in\"] > flows.out", "flows[\"in\"] > flows.out"),
    // `record` is written only where a key needs it.
    (
        "record.null == 1 and record[\"in\"] == 2",
        "null == 1 and record[\"in\"] == 2",
    ),
    // One chain of `and` is one list, however it is grouped.
    ("(a and b) and (c or d) and e", "a and b and (c or d) and e"),
    // A list in square brackets, and a text in single quotes.
    ("x in [expected, 'paid']", "x in (expected, \"paid\")"),
    // A rule over several lines.
    ("a > 1 and\n  b < 2", "a > 1 and b < 2"),
    (
        "not (a == 1 or b != 2) and c <= -3.5",
        "not (a == 1 or b != 2) and c <= -3.5",
    ),
    // A hexadecimal integer and underscores in a number.
    (
        "record[\"unit price\"] * qty + 1 >= 0x10 - 1_000 % 7",
        "record[\"unit price\"] * qty + 1 >= 16 - 1000 % 7",
    ),
    ("0x10 + 1_000 > .5", "16 + 1000 > 0.5"),
    (
        "(price if price is not None else 0) / 2 < limit",
        "(price if price is not None else 0) / 2 < limit",
    ),
    // The smallest integer.
    ("-9223372036854775808 < x", "-9223372036854775808 < x"),
    // The minus as a literal and as `Neg`.
    (
        "a - -5 > -(5) and -a * b > 0",
        "a - -5 > -(5) and -a * b > 0",
    ),
    (
        "customer.country == \"NL\" and customer.email.endswith(\"@example.com\")",
        "customer.country == \"NL\" and customer.email.endswith(\"@example.com\")",
    ),
    (
        "len(name.strip().upper()) > 3",
        "len(name.strip().upper()) > 3",
    ),
    (
        "re.search(r\"^\\d{5}$\", zip_code) and isinstance(price, (int, float))",
        "re.search(r\"^\\d{5}$\", zip_code) and isinstance(price, (int, float))",
    ),
    // A comma after the last item of a list.
    (
        "status in (expected, \"paid\",)\n  and math.isnan(ratio)\n  or flag",
        "status in (expected, \"paid\") and math.isnan(ratio) or flag",
    ),
    // A function name without `(` is a field.
    ("len == 1", "len == 1"),
    // A field named like a module.
    ("record[\"math\"] > 50", "record[\"math\"] > 50"),
    // A list of one, the empty list, and one type in parentheses.
    (
        "(a,) == () or isinstance(x, (int,))",
        "(a,) == () or isinstance(x, int)",
    ),
    // A pattern that cannot be written raw.
    ("re.match(\"a\\\"b\", x)", "re.match(\"a\\\"b\", x)"),
    (
        "any(i.price > 100 for i in items)",
        "any(i.price > 100 for i in items)",
    ),
    (
        "all(len(t) > 0 and t.isascii() for t in tags) and \"vip\" in tags",
        "all(len(t) > 0 and t.isascii() for t in tags) and \"vip\" in tags",
    ),
    (
        "any(any(l.qty > i.max for l in i.lines) for i in items)",
        "any(any(l.qty > i.max for l in i.lines) for i in items)",
    ),
    // A field of the record under an element of the same name.
    (
        "any(i > record.i for i in items)",
        "any(i > record.i for i in items)",
    ),
];

/// The reason "expected `wanted` after `after`, found `found`".
fn expected(wanted: &[&str], found: Found, after: Option<&str>) -> Reason {
    Reason::Expected {
        expected: wanted.iter().map(ToString::to_string).collect(),
        found,
        after: after.map(str::to_owned),
        nearest: None,
    }
}

/// The reason for a name that stands where `wanted` could, after `after`.
fn found_name(wanted: &[&str], name: &str, after: Option<&str>) -> Reason {
    expected(wanted, Found::Token(name.to_owned()), after)
}

/// (text, the bytes the error is about, the reason, its sentence).
#[expect(clippy::too_many_lines, reason = "one row per faulty text")]
fn faulty() -> Vec<(&'static str, Range<usize>, Reason, &'static str)> {
    let foreign = |found, write| Reason::ForeignOperator { found, write };
    let after_condition = ["`and`", "`or`", "`if`", "the end of the rule"];
    let count = |name, takes, given| Reason::WrongArgumentCount { name, takes, given };
    vec![
        (
            "",
            0..0,
            Reason::Empty,
            "a rule is empty; write an expression",
        ),
        (
            "amount >",
            8..8,
            expected(&["a value"], Found::End, Some(">")),
            "expected a value after `>`, found the end of the rule",
        ),
        (
            "amount = 100",
            7..8,
            foreign("=", "=="),
            "`=` is not an operator here; write `==`",
        ),
        (
            "a > 1 && b < 2",
            6..8,
            foreign("&&", "and"),
            "`&&` is not an operator here; write `and`",
        ),
        (
            "a > 1 || b < 2",
            6..8,
            foreign("||", "or"),
            "`||` is not an operator here; write `or`",
        ),
        (
            "!active",
            0..1,
            foreign("!", "not"),
            "`!` is not an operator here; write `not`",
        ),
        (
            "status <> \"paid\"",
            7..9,
            foreign("<>", "!="),
            "`<>` is not an operator here; write `!=`",
        ),
        (
            "x == None",
            2..9,
            Reason::NoneComparison {
                op: "==",
                write: "x is None".to_owned(),
            },
            "`==` does not test for a missing value; write `x is None`",
        ),
        (
            "0 < amount < 10",
            11..12,
            Reason::ChainedComparison {
                write: Some("0 < amount and amount < 10".to_owned()),
            },
            "chained comparisons are not supported; write `0 < amount and amount < 10`",
        ),
        (
            "items[-1].price > 5",
            6..8,
            Reason::NegativeIndex,
            "a negative index is not supported",
        ),
        (
            "(a > 1 and b < 2",
            0..1,
            Reason::NeverClosed { open: '(' },
            "this `(` is never closed",
        ),
        (
            "a > 1) and b",
            5..6,
            Reason::ClosesNothing { close: ')' },
            "this `)` closes nothing",
        ),
        (
            "name == \"abc",
            8..12,
            Reason::UnclosedText,
            "this text is never closed",
        ),
        (
            "amount > 100 # big ones",
            13..23,
            Reason::Comment,
            "comments are not supported in a rule",
        ),
        (
            "x in (\"a\", \"b\"",
            5..6,
            Reason::NeverClosed { open: '(' },
            "this `(` is never closed",
        ),
        (
            "f\"{a}\" == \"x\"",
            0..6,
            Reason::TextPrefix {
                prefix: "f".to_owned(),
            },
            "the text prefix `f` is not supported; only `r` is",
        ),
        (
            "amount > 1e999",
            9..14,
            Reason::FloatTooLarge,
            "this number is too large for a float",
        ),
        (
            "amount > 99999999999999999999",
            9..29,
            Reason::IntTooLarge,
            "this number is too large for an integer",
        ),
        (
            "name == 'a\\qb'",
            10..12,
            Reason::BadEscape {
                found: "\\q".to_owned(),
            },
            "unknown escape `\\q` in a text",
        ),
        (
            "customer..country == 1",
            9..10,
            found_name(&["a name"], ".", Some(".")),
            "expected a name after `.`, found `.`",
        ),
        (
            "record == 1",
            0..6,
            Reason::BareRecord,
            "`record` is the whole record; name a field, as in `record[\"key\"]`",
        ),
        (
            "a > 1 b < 2",
            6..7,
            found_name(&after_condition, "b", Some("a > 1")),
            "expected `and`, `or`, `if` or the end of the rule after `a > 1`, found `b`",
        ),
        (
            "price * > 3",
            8..9,
            found_name(&["a value"], ">", Some("*")),
            "expected a value after `*`, found `>`",
        ),
        (
            "price if price > 0",
            18..18,
            Reason::ConditionalNeedsElse,
            "a conditional needs `else`; write `a if test else b`",
        ),
        (
            "lenn(name) > 3",
            0..4,
            Reason::UnknownFunction {
                name: "lenn".to_owned(),
                nearest: Some("len"),
            },
            "unknown function `lenn`; did you mean `len`?",
        ),
        (
            "abs(a - b) > 1",
            0..3,
            Reason::UnknownFunction {
                name: "abs".to_owned(),
                nearest: None,
            },
            "unknown function `abs`; the functions are len, any, all, isinstance, re.search, \
             re.match, re.fullmatch, math.isnan",
        ),
        (
            "name.startwith(\"a\")",
            5..14,
            Reason::UnknownMethod {
                name: "startwith".to_owned(),
                nearest: Some("startswith"),
            },
            "unknown method `startwith`; did you mean `startswith`?",
        ),
        (
            "len(a, b) > 1",
            3..9,
            count("len", 1, 2),
            "`len` takes 1 argument, and 2 were given",
        ),
        (
            "name.upper(\"x\") == \"A\"",
            10..15,
            count("upper", 0, 1),
            "`upper` takes no arguments",
        ),
        (
            "isinstance(price, number)",
            18..24,
            Reason::UnknownType {
                name: "number".to_owned(),
                nearest: None,
            },
            "unknown type `number`; the types are int, float, str, bool, list",
        ),
        (
            "any(i.price > 100 for i items)",
            24..29,
            found_name(&["`in`"], "items", Some("i")),
            "expected `in` after `i`, found `items`",
        ),
        (
            "any(i.price > 100)",
            17..18,
            Reason::QuantifierNeedsFor { function: "any" },
            "`any` needs `for <name> in <list>` after its test",
        ),
        (
            "a > 1 AND b < 2",
            6..9,
            Reason::KeywordCase {
                found: "AND".to_owned(),
                keyword: "and",
            },
            "keywords are lowercase: write `and`",
        ),
        (
            "lambda x: x > 1",
            0..6,
            Reason::NotInLanguage {
                word: "lambda".to_owned(),
            },
            "`lambda` is not part of the rule language",
        ),
    ]
}

proptest! {
    // No regression file, so a failure leaves nothing untracked in the tree.
    // The failing seed is in the output.
    #![proptest_config(ProptestConfig {
        failure_persistence: None,
        .. ProptestConfig::default()
    })]

    /// A canonical tree prints to a text that parses back to the same tree.
    #[test]
    fn a_printed_tree_parses_back_to_itself(tree in canonical_in(&[], 3)) {
        let printed = tree.to_string();
        prop_assert_eq!(parse(&printed), Ok(tree), "{}", printed);
    }
}

#[test]
fn a_valid_text_prints_canonically_and_parses_back() {
    for (text, printed) in VALID {
        let tree = parse(text).unwrap_or_else(|e| panic!("{text:?} is refused: {e}"));
        assert_eq!(tree.to_string(), *printed, "{text:?}");
        // The printed text parses to the same tree as the text, so the
        // printed column is itself a valid rule.
        assert_eq!(parse(printed).as_ref(), Ok(&tree), "{text:?}");
    }
}

#[test]
fn a_faulty_text_is_refused_at_its_place_with_its_reason() {
    for (text, at, reason, sentence) in faulty() {
        let error = parse(text).expect_err(text);
        assert_eq!((&error.at, &error.reason), (&at, &reason), "{text:?}");
        // The sentence is the reason's `Display`, so the row shows an
        // author's whole view of the mistake.
        assert_eq!(error.to_string(), sentence, "{text:?}");
    }
}
