//! The rule language as text: two tables. The valid texts pin the spellings
//! of the language. The faulty texts are the specification of the errors:
//! the place, the reason and the sentence of each.

use std::ops::Range;

use trapperkeeper_engine::parse;
use trapperkeeper_engine::text::{Found, Reason};

/// (text, its canonical spelling). Both parse to the same tree.
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
];

/// The reason "expected `wanted` after `after`, found `found`".
fn expected(wanted: &[&str], found: Found, after: Option<&str>) -> Reason {
    Reason::Expected {
        expected: wanted.iter().map(ToString::to_string).collect(),
        found,
        after: after.map(str::to_owned),
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
    ]
}

#[test]
fn a_valid_text_parses_as_its_canonical_spelling() {
    for (text, canonical) in VALID {
        let tree = parse(text).unwrap_or_else(|e| panic!("{text:?} is refused: {e}"));
        assert_eq!(parse(canonical).as_ref(), Ok(&tree), "{text:?}");
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
