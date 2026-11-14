//! The rule language as text: two properties, and two tables for what a
//! property cannot say. The valid texts pin spellings that the printer never
//! writes. The faulty texts are the specification of the errors: the place,
//! the reason and the sentence of each.

use std::ops::Range;

use proptest::collection::vec;
use proptest::prelude::*;
use proptest::sample::select;
use trapperkeeper_engine::expr::{Expr, Segment, TypeName};
use trapperkeeper_engine::limits::{MAX_NESTING, MAX_TEXT_LENGTH};
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

/// Half arbitrary text, and half a printed canonical tree with one character
/// deleted, replaced or inserted, so most cases get past the tokenizer.
fn mixed_text() -> impl Strategy<Value = String> {
    let edited = (
        canonical_in(&[], 3),
        any::<prop::sample::Index>(),
        proptest::option::of(any::<char>()),
        any::<bool>(),
    )
        .prop_map(|(tree, at, with, keep)| {
            let mut chars: Vec<char> = tree.to_string().chars().collect();
            let at = at.index(chars.len() + 1);
            // `keep` inserts `with` before the character at `at`. Without
            // it the character is replaced by `with`, or deleted.
            if !keep && at < chars.len() {
                chars.remove(at);
            }
            if let Some(c) = with {
                chars.insert(at.min(chars.len()), c);
            }
            chars.into_iter().collect()
        });
    prop_oneof![any::<String>(), edited]
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
    let after_operand = [
        "an operator",
        "`and`",
        "`or`",
        "`if`",
        "the end of the rule",
    ];
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
        (
            "a and or b",
            6..8,
            expected(&["a value"], Found::Word("or"), Some("and")),
            "expected a value after `and`, found `or`; to name a field `or`, write \
             `record[\"or\"]`",
        ),
        (
            "a adn b",
            2..5,
            Reason::Expected {
                expected: after_operand.map(str::to_owned).to_vec(),
                found: Found::Token("adn".to_owned()),
                after: Some("a".to_owned()),
                nearest: Some("and"),
            },
            "expected an operator, `and`, `or`, `if` or the end of the rule after `a`, found \
             `adn`; did you mean `and`?",
        ),
        // `iz` is one edit from `is`, from `in` and from `if`, so none is
        // offered.
        (
            "x iz None",
            2..4,
            found_name(&after_operand, "iz", Some("x")),
            "expected an operator, `and`, `or`, `if` or the end of the rule after `x`, found `iz`",
        ),
        (
            "x iss None",
            2..5,
            Reason::Expected {
                expected: after_operand.map(str::to_owned).to_vec(),
                found: Found::Token("iss".to_owned()),
                after: Some("x".to_owned()),
                nearest: Some("is"),
            },
            "expected an operator, `and`, `or`, `if` or the end of the rule after `x`, found \
             `iss`; did you mean `is`?",
        ),
        (
            "[1, 2) == x",
            5..6,
            found_name(
                &["an operator", "`and`", "`or`", "`if`", "`,`", "`]`"],
                ")",
                Some("2"),
            ),
            "expected an operator, `and`, `or`, `if`, `,` or `]` after `2`, found `)`",
        ),
        (
            "x not y",
            6..7,
            found_name(&["`in`"], "y", Some("not")),
            "expected `in` after `not`, found `y`",
        ),
        (
            "x is 5",
            5..6,
            found_name(&["`not`", "`None`"], "5", Some("is")),
            "expected `not` or `None` after `is`, found `5`",
        ),
        (
            "x is none",
            5..9,
            Reason::KeywordCase {
                found: "none".to_owned(),
                keyword: "None",
            },
            "write `None` with a capital letter",
        ),
        (
            "re.serch(r\"a\", x)",
            0..8,
            Reason::UnknownFunction {
                name: "re.serch".to_owned(),
                nearest: Some("re.search"),
            },
            "unknown function `re.serch`; did you mean `re.search`?",
        ),
        (
            "re.upper()",
            0..8,
            Reason::UnknownFunction {
                name: "re.upper".to_owned(),
                nearest: None,
            },
            "unknown function `re.upper`; the functions are len, any, all, isinstance, \
             re.search, re.match, re.fullmatch, math.isnan",
        ),
        (
            "re == 1",
            0..2,
            Reason::ModuleAlone { word: "re" },
            "`re` names the pattern functions; to name a field `re`, write `record[\"re\"]`",
        ),
        (
            "math > 50",
            0..4,
            Reason::ModuleAlone { word: "math" },
            "`math` names the math functions; to name a field `math`, write `record[\"math\"]`",
        ),
        (
            "len() > 1",
            3..5,
            count("len", 1, 0),
            "`len` takes 1 argument, and 0 were given",
        ),
        (
            "isinstance(x)",
            10..13,
            count("isinstance", 2, 1),
            "`isinstance` takes 2 arguments, and 1 was given",
        ),
        (
            "a === b",
            4..5,
            foreign("=", "=="),
            "`=` is not an operator here; write `==`",
        ),
        (
            "amount > 100;",
            12..13,
            Reason::UnexpectedCharacter { found: ';' },
            "unexpected character `;`",
        ),
        (
            "amount > $100",
            9..10,
            Reason::UnexpectedCharacter { found: '$' },
            "unexpected character `$`",
        ),
        (
            "a > 1 and",
            9..9,
            expected(&["a value"], Found::End, Some("and")),
            "expected a value after `and`, found the end of the rule",
        ),
        (
            "all(x > 1 for in items)",
            14..16,
            expected(&["a name"], Found::Word("in"), Some("for")),
            "expected a name after `for`, found `in`; to name a field `in`, write \
             `record[\"in\"]`",
        ),
        (
            "price > 10 else 5",
            11..15,
            expected(&after_condition, Found::Word("else"), Some("price > 10")),
            "expected `and`, `or`, `if` or the end of the rule after `price > 10`, found \
             `else`; to name a field `else`, write `record[\"else\"]`",
        ),
        (
            "\"abc\" \"def\" == name",
            6..11,
            Reason::AdjacentTexts,
            "two texts side by side are not joined; write one text",
        ),
        (
            "\"\"\"abc\"\"\" == x",
            0..3,
            Reason::TripleQuote,
            "triple-quoted texts are not supported; write one pair of quotes",
        ),
        (
            "amount > 1_",
            9..11,
            Reason::MalformedNumber,
            "malformed number",
        ),
        ("0x > 1", 0..2, Reason::MalformedNumber, "malformed number"),
        (
            "9223372036854775808 > x",
            0..19,
            Reason::IntTooLarge,
            "this number is too large for an integer",
        ),
        (
            "a < b <",
            6..7,
            Reason::ChainedComparison { write: None },
            "chained comparisons are not supported",
        ),
        (
            "name == \"\\u12\"",
            9..13,
            Reason::BadEscape {
                found: "\\u12".to_owned(),
            },
            "unknown escape `\\u12` in a text",
        ),
        (
            "None == x",
            0..7,
            Reason::NoneComparison {
                op: "==",
                write: "x is None".to_owned(),
            },
            "`==` does not test for a missing value; write `x is None`",
        ),
        (
            "x != None",
            2..9,
            Reason::NoneComparison {
                op: "!=",
                write: "x is not None".to_owned(),
            },
            "`!=` does not test for a missing value; write `x is not None`",
        ),
        (
            "a > 1 and\n  b >\n  or c",
            18..20,
            expected(&["a value"], Found::Word("or"), Some(">")),
            "expected a value after `>`, found `or`; to name a field `or`, write \
             `record[\"or\"]`",
        ),
        (
            "in > 5",
            0..2,
            expected(&["an expression"], Found::Word("in"), None),
            "expected an expression, found `in`; to name a field `in`, write `record[\"in\"]`",
        ),
        (
            "flows.in > 3",
            6..8,
            Reason::WordAfterDot {
                word: "in".to_owned(),
                write: "flows[\"in\"]".to_owned(),
            },
            "`in` is a word of the rule language; write `flows[\"in\"]`",
        ),
        (
            "re.search(pattern, x)",
            10..17,
            Reason::PatternNotLiteral {
                function: "re.search",
            },
            "the pattern of `re.search` must be a text literal, as in \
             `re.search(r\"^a\", name)`",
        ),
        (
            "\"abc\".x == 1",
            5..7,
            Reason::KeyOnValue,
            "only a field has keys and indexes",
        ),
        (
            "record[0] == 1",
            0..9,
            Reason::BareRecord,
            "`record` is the whole record; name a field, as in `record[\"key\"]`",
        ),
        (
            "class x",
            0..5,
            Reason::NotInLanguage {
                word: "class".to_owned(),
            },
            "`class` is not part of the rule language",
        ),
    ]
}

/// A number for each case of `Reason`, so the table of faulty texts can be
/// checked for a row of every case.
fn case(reason: &Reason) -> usize {
    // One arm per case and no wildcard, so a new reason does not compile
    // until it has a number here.
    match reason {
        Reason::TooLong { .. } => 0,
        Reason::TooDeep { .. } => 1,
        Reason::TooComplex => 2,
        Reason::UnexpectedCharacter { .. } => 3,
        Reason::ForeignOperator { .. } => 4,
        Reason::Comment => 5,
        Reason::UnclosedText => 6,
        Reason::TripleQuote => 7,
        Reason::TextPrefix { .. } => 8,
        Reason::BadEscape { .. } => 9,
        Reason::MalformedNumber => 10,
        Reason::IntTooLarge => 11,
        Reason::FloatTooLarge => 12,
        Reason::Empty => 13,
        Reason::Expected { .. } => 14,
        Reason::NeverClosed { .. } => 15,
        Reason::ClosesNothing { .. } => 16,
        Reason::KeywordCase { .. } => 17,
        Reason::NotInLanguage { .. } => 18,
        Reason::WordAfterDot { .. } => 19,
        Reason::BareRecord => 20,
        Reason::ModuleAlone { .. } => 21,
        Reason::KeyOnValue => 22,
        Reason::NegativeIndex => 23,
        Reason::ChainedComparison { .. } => 24,
        Reason::NoneComparison { .. } => 25,
        Reason::AdjacentTexts => 26,
        Reason::UnknownFunction { .. } => 27,
        Reason::UnknownMethod { .. } => 28,
        Reason::UnknownType { .. } => 29,
        Reason::WrongArgumentCount { .. } => 30,
        Reason::PatternNotLiteral { .. } => 31,
        Reason::QuantifierNeedsFor { .. } => 32,
        Reason::ConditionalNeedsElse => CASES - 1,
    }
}

/// How many cases `Reason` has. It is the number of arms of `case`, so it
/// grows with the enum.
const CASES: usize = 34;

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

    /// Any text gets a tree or an error, and the error's place is a range of
    /// whole characters inside the text.
    #[test]
    fn any_text_gets_a_tree_or_an_error_inside_it(text in mixed_text()) {
        if let Err(error) = parse(&text) {
            // `get` gives nothing for a range that leaves the text or that
            // cuts a character.
            prop_assert!(text.get(error.at.clone()).is_some(), "{:?}", error);
        }
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
    let mut seen = [false; CASES];
    for (text, at, reason, sentence) in faulty() {
        let error = parse(text).expect_err(text);
        assert_eq!((&error.at, &error.reason), (&at, &reason), "{text:?}");
        // The sentence is the reason's `Display`, so the row shows an
        // author's whole view of the mistake.
        assert_eq!(error.to_string(), sentence, "{text:?}");
        seen[case(&reason)] = true;
    }
    // Every case of `Reason` has at least one row. The two limits have their
    // texts in `the_limits_hold_at_their_edges`, because those are too long
    // for a table, and no text reaches `TooComplex` while no two
    // alternatives of the grammar read the same bracketed text.
    let elsewhere = [
        Reason::TooLong { max: 0 },
        Reason::TooDeep { max: 0 },
        Reason::TooComplex,
    ];
    for reason in &elsewhere {
        seen[case(reason)] = true;
    }
    let missing: Vec<usize> = (0..CASES).filter(|n| !seen[*n]).collect();
    assert!(
        missing.is_empty(),
        "no faulty text for the cases {missing:?}"
    );
}

#[test]
fn the_limits_hold_at_their_edges() {
    // A rule of plain conditions, padded with spaces to the cap.
    let conditions = format!("a > 1{}", " and a > 1".repeat(400));
    let longest = format!("{conditions:<MAX_TEXT_LENGTH$}");
    assert!(parse(&longest).is_ok());
    let error = parse(&format!("{longest} ")).expect_err("a text over the cap");
    let max = MAX_TEXT_LENGTH;
    assert_eq!(error.reason, Reason::TooLong { max });
    assert_eq!(error.at, max..max + 1);
    assert_eq!(error.to_string(), "a rule is at most 4096 characters long");

    let nested = |levels: usize| format!("{}x{}", "(".repeat(levels), ")".repeat(levels));
    assert!(parse(&nested(MAX_NESTING)).is_ok());
    let error = parse(&nested(MAX_NESTING + 1)).expect_err("a text nested over the cap");
    let max = MAX_NESTING;
    assert_eq!(error.reason, Reason::TooDeep { max });
    // The error points at the opening bracket that is one too many.
    assert_eq!(error.at, max..max + 1);
    assert_eq!(error.to_string(), "a rule nests at most 32 levels deep");
}

#[test]
fn a_deep_faulty_text_is_refused_for_its_own_mistake() {
    // (what opens a level, what closes it, how many levels of nesting one
    // level of the form takes)
    let forms = [
        ("(", ")", 1),
        ("[", "]", 1),
        ("x in (1, ", ")", 1),
        ("a == (", ")", 1),
        ("len(", ")", 1),
        ("math.isnan(", ")", 1),
        ("any(", " for i in xs)", 1),
        ("all(", " for i in xs)", 1),
        ("isinstance(", ", int)", 1),
        ("re.search(r\"a\", ", ")", 1),
        ("re.match(r\"a\", ", ")", 1),
        ("re.fullmatch(r\"a\", ", ")", 1),
        ("x.startswith(", ")", 1),
        ("items[0].endswith(", ")", 1),
        ("(1 if ", " else 2)", 1),
        ("(", " if a else 2)", 1),
        ("not (", ")", 2),
        ("-(", ")", 2),
        ("(a if b else ", ")", 2),
    ];
    for (open, close, levels) in forms {
        // Each form is nested 30 levels deep, with the mistake `x y` at the
        // innermost place.
        let depth = 30 / levels;
        let text = format!("{}x y{}", open.repeat(depth), close.repeat(depth));
        let mistake = open.len() * depth + 2;
        let error = parse(&text).expect_err("a faulty text");
        // `TooComplex` here means that two alternatives of the grammar read
        // the same bracket, which `engine/src/text/AGENTS.md` forbids.
        assert_ne!(error.reason, Reason::TooComplex, "{open:?}");
        assert_eq!(error.at, mistake..mistake + 1, "{open:?}");
    }
}
