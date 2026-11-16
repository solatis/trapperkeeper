//! Tests of the parser and the printer of rule text. Two properties check
//! that a printed canonical tree parses back to itself, and that any text
//! gets a tree or an error inside it. Two tables hold valid texts with their
//! printed form and faulty texts with their error. Three tests cover the
//! limits: the caps at their edges, deep faulty texts, and the largest
//! canonical tree under the cap on length.

use std::ops::Range;
use std::sync::LazyLock;

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

/// The kinds of node with one operand that the parser builds around any
/// operand.
const UNARY: [Unary; 17] = [
    Expr::Not,
    Expr::IsNone,
    Expr::IsNotNone,
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

/// The kinds of node with two operands that the parser builds around any
/// two operands.
const BINARY: [Binary; 4] = [Expr::Eq, Expr::Ne, Expr::StartsWith, Expr::EndsWith];

/// The kinds of node with two operands that refuse the literal `None` as
/// their right operand.
const MEMBERSHIP: [Binary; 2] = [Expr::In, Expr::NotIn];

/// The kinds of node with two operands that refuse the literal `None` as an
/// operand: the orderings and the arithmetic.
const NUMERIC: [Binary; 9] = [
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

/// Every variant of `TypeName`, the types that `isinstance` tests for. The
/// strategy picks one to three of them.
const TYPES: [TypeName; 5] = [
    TypeName::Int,
    TypeName::Float,
    TypeName::Str,
    TypeName::Bool,
    TypeName::List,
];

/// The words of the language, which no name can be. The list copies `WORDS`
/// of the grammar, the twelve words of ADR-0041 and the two module names. The
/// grammar's constant is private to its crate and an integration test cannot
/// import it, so a word added to the grammar is added here too.
const WORDS: [&str; 14] = [
    "and", "or", "not", "in", "is", "if", "else", "for", "None", "True", "False", "record", "re",
    "math",
];

/// The steps of a generated field after its first key, at most. Two keeps
/// the 27 fields of the largest tree, with keys of eight characters, under
/// `MAX_TEXT_LENGTH`.
const MOST_FIELD_STEPS: usize = 2;

/// A name that an author can write bare: a letter or `_`, then letters,
/// digits or `_`, and no word of the language. A Python keyword such as
/// `class` and a miscased word such as `AND` are names too. The rows
/// `a > 1 AND b < 2` and `class x` of `faulty` are refused because the parse
/// fails at such a name or right after it.
fn name() -> impl Strategy<Value = String> {
    "[a-zA-Z_][a-zA-Z0-9_]{0,7}"
        .prop_filter("a word of the language", |n| !WORDS.contains(&n.as_str()))
}

/// The regex from which proptest generates a text of at most eight
/// characters, line breaks included.
fn text() -> &'static str {
    "(?s).{0,8}"
}

/// A canonical tree of at most `depth` levels above its leaves, in which the
/// names of `scope` are bound elements. A canonical tree is one the parser
/// can build.
fn canonical_in(scope: &[String], depth: u32) -> BoxedStrategy<Expr> {
    // The function builds a strategy for trees that the parser can build.
    // Each level offers a leaf, or one node whose operands come from the
    // strategy one level less deep. The function calls itself with `depth`
    // and `scope` and does not use `prop_recursive`, because the test of a
    // quantifier needs the strategy for a larger scope, with its own name
    // added, so that a test can hold the elements of every enclosing
    // quantifier.
    //
    // Lists and operand lists hold at most three items before `and` and `or`
    // splice, keys and texts at most eight characters, and a field at most
    // three segments, so depth 3 gives at most 27 leaves. A key of eight
    // control characters prints as 32 characters of escapes plus its quotes
    // and brackets. The 27 leaves with their operators stay near 3300
    // characters, under `MAX_TEXT_LENGTH`, and the three levels stay under
    // `MAX_NESTING`.
    //
    // 1. We build the leaf strategies: `None`, a bool, any i64, a finite
    //    float, a text, a field whose first segment is a key, and an element
    //    named from `scope` when `scope` is not empty. We join them into
    //    `leaf` and return `leaf` alone at depth 0. The leaves are the whole
    //    tree at depth 0 and one alternative at every other depth, so they
    //    are built before the guard.
    // 2. We build `sub`, the strategy for operands one level less deep, and
    //    the builders that several kinds of node share: one boxed operand,
    //    an `and` or `or` chain, an operand that is not the literal `None`,
    //    a quantifier and a pattern function. The chain and the quantifier
    //    build the tree in the form the parser builds, so a printed tree
    //    parses back to itself.
    // 3. We choose among the leaves, the lists and every kind of node the
    //    parser builds, each over the operands of step 2.

    // 1. build the leaves
    // The closure generates up to `most` steps. A step key is any text, and
    // an index is any u32, which `bracket_step` reads up to `u32::MAX`.
    let steps = |most: usize| {
        let step = prop_oneof![
            text().prop_map(Segment::Key),
            any::<u32>().prop_map(Segment::Index),
        ];
        vec(step, 0..=most)
    };
    // The strategy generates finite floats only, because a tree holds no
    // other float. The tokenizer refuses `1e999`, and no literal writes a
    // NaN.
    let float = {
        use proptest::num::f64::{NEGATIVE, NORMAL, POSITIVE, SUBNORMAL, ZERO};
        NEGATIVE | NORMAL | POSITIVE | SUBNORMAL | ZERO
    };
    // A field has one to three segments, and the first is a key, because the
    // record is an object. The parser refuses `record[0] == 1` and
    // `record == 1`. The first key is any text, so the printer writes some
    // fields as `record["..."]`.
    let field = (text(), steps(MOST_FIELD_STEPS)).prop_map(|(first, rest)| {
        let key = Segment::Key(first);
        Expr::Field(std::iter::once(key).chain(rest).collect())
    });
    let mut leaves = vec![
        Just(Expr::None).boxed(),
        any::<bool>().prop_map(Expr::Bool).boxed(),
        // The strategy generates any i64, because the parser reads
        // `-9223372036854775808` back as the smallest i64.
        any::<i64>().prop_map(Expr::Int).boxed(),
        float.prop_map(Expr::Float).boxed(),
        text().prop_map(Expr::Text).boxed(),
        field.boxed(),
    ];
    // An element is one of the names of `scope`, because an element exists
    // only under the quantifier that names it. Outside every quantifier a
    // bare name reads back as a field, as `amount` does.
    if !scope.is_empty() {
        // An element gets at most two steps, which is a choice that no
        // measurement backs. It prints shorter than a field of three keys,
        // so the bound on the length of a printed tree holds.
        let element = (select(scope.to_vec()), steps(2));
        leaves.push(
            element
                .prop_map(|(name, path)| Expr::Element { name, path })
                .boxed(),
        );
    }
    let leaf = proptest::strategy::Union::new(leaves);
    let Some(below) = depth.checked_sub(1) else {
        return leaf.boxed();
    };

    // 2. build the operands and the shared builders
    let sub = canonical_in(scope, below);
    let one = || sub.clone().prop_map(Box::new);
    // `and` and `or` take two or three operands, and an operand of the same
    // kind is spliced in, as the parser does: `(a and b) and c` reads back
    // as one `And` of three.
    let chain = |make: fn(Vec<Expr>) -> Expr| {
        vec(sub.clone(), 2..=3).prop_map(move |operands| {
            // `make` is a function pointer, and Rust does not promise that
            // two pointers to one function compare equal, so the closure
            // learns the kind it builds from an empty node.
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
    let not_none = || one().prop_filter("the literal None", |e| !matches!(**e, Expr::None));
    // The test of a quantifier is generated with its name added to `scope`,
    // and its list without it. The parser resolves the list outside the
    // name, so `any(t for i in i)` reads its list `i` as a field. The test
    // depends on the name, so `prop_flat_map` builds it per name.
    let quantifier = |all: bool| {
        let (scope, list) = (scope.to_vec(), sub.clone());
        name().prop_flat_map(move |name| {
            let inner: Vec<String> = scope.iter().cloned().chain([name.clone()]).collect();
            let test = canonical_in(&inner, below);
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
    // Any text is a pattern, because the parser does not check a pattern.
    let pattern = |make: fn(String, Box<Expr>) -> Expr| {
        (text(), one()).prop_map(move |(pattern, text)| make(pattern, text))
    };
    // 3. choose among the kinds of node
    prop_oneof![
        leaf,
        // `()`, `(a,)` and longer lists all read back.
        vec(sub.clone(), 0..=3).prop_map(Expr::List),
        (select(UNARY.to_vec()), one()).prop_map(|(make, a)| make(a)),
        (select(BINARY.to_vec()), one(), one()).prop_map(|(make, a, b)| make(a, b)),
        // The right operand is not the literal `None`, because the parser
        // refuses `x in None`. `None in x` reads back.
        (select(MEMBERSHIP.to_vec()), one(), not_none()).prop_map(|(make, a, b)| make(a, b)),
        // No operand is the literal `None`, because the parser refuses
        // `x < None` and `None + 1`.
        (select(NUMERIC.to_vec()), not_none(), not_none()).prop_map(|(make, a, b)| make(a, b)),
        // The operand is not the literal `None`, because the parser refuses
        // `-None`.
        not_none().prop_map(Expr::Neg),
        chain(Expr::And),
        chain(Expr::Or),
        (one(), one(), one()).prop_map(|(test, then, otherwise)| Expr::If {
            test,
            then,
            otherwise
        }),
        quantifier(false),
        quantifier(true),
        // The strategy generates one to three types, because the parser
        // refuses `isinstance(x, ())`. Three is the bound of every list
        // here, and a repeated type reads back as written.
        (one(), vec(select(TYPES.to_vec()), 1..=3))
            .prop_map(|(value, types)| Expr::IsInstance { value, types }),
        pattern(|pattern, text| Expr::ReSearch { pattern, text }),
        pattern(|pattern, text| Expr::ReMatch { pattern, text }),
        pattern(|pattern, text| Expr::ReFullmatch { pattern, text }),
    ]
    .boxed()
}

/// Half of the cases are arbitrary text. The other half are a printed
/// canonical tree with one character deleted, replaced or inserted, or left
/// as it is, so that part of the cases reach the parser.
fn mixed_text() -> impl Strategy<Value = String> {
    let edited = (
        canonical_in(&[], 3),
        any::<prop::sample::Index>(),
        proptest::option::of(any::<char>()),
        any::<bool>(),
    )
        .prop_map(|(tree, at, with, keep)| {
            // The edit works on characters, so it never cuts one in two.
            let mut chars: Vec<char> = tree.to_string().chars().collect();
            // The place runs from 0 up to and including the length, so an
            // insertion can also go at the end.
            let at = at.index(chars.len() + 1);
            // Without `keep`, the character at `at` is removed, so that
            // `with` replaces it below or it is deleted. At the end of the
            // text no character is removed.
            if !keep && at < chars.len() {
                chars.remove(at);
            }
            // With `keep`, `with` goes in before the character at `at`. At
            // the end of the text it is appended. With `keep` and no `with`,
            // the printed tree stays as it is.
            if let Some(c) = with {
                chars.insert(at, c);
            }
            chars.into_iter().collect()
        });
    // Arbitrary text mostly stops at the tokenizer, often at a character
    // of several bytes.
    prop_oneof![any::<String>(), edited]
}

/// Each row holds a text that the parser accepts and the text its tree
/// prints as. The second column pins the spelling that the printer writes. A
/// row whose columns differ also shows a spelling that the parser accepts and
/// the printer never writes, as `[...]` and single quotes.
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
    // `==` and `!=` with `None` test for a missing value, with `None` on
    // either side.
    ("x == None", "x == None"),
    ("None == x", "None == x"),
    ("x != None", "x != None"),
    // A `None` in a membership list matches a missing value.
    ("x in (\"paid\", None)", "x in (\"paid\", None)"),
    // The printer writes every text in double quotes and escapes only the
    // backslash, the double quote and control characters. So the `\'` of the
    // single-quoted text loses its backslash, and the line break stays the
    // escape `\n`.
    (
        "items[0].lines[2].sku == 'a\\'b\\n'",
        "items[0].lines[2].sku == \"a'b\\n\"",
    ),
    // A lowercase `null` or `true` is a field.
    ("status == null", "status == null"),
    ("active == true", "active == true"),
    // A Python keyword that is no word of the language is a field.
    (
        "from == \"AMS\" and class == 1",
        "from == \"AMS\" and class == 1",
    ),
    // The bracket form, at the record and at a later step.
    ("record[\"unit price\"] > 2", "record[\"unit price\"] > 2"),
    ("flows[\"in\"] > flows.out", "flows[\"in\"] > flows.out"),
    // The printer writes `record` only before a key that cannot stand as a
    // bare name. `null` can, and `in` is a word of the language.
    (
        "record.null == 1 and record[\"in\"] == 2",
        "null == 1 and record[\"in\"] == 2",
    ),
    // One chain of `and` is one `And` node, however it is grouped.
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
    // In a hexadecimal integer `e` is a digit, and no exponent.
    ("0x1e_5 > x", "485 > x"),
    (
        "(price if price is not None else 0) / 2 < limit",
        "(price if price is not None else 0) / 2 < limit",
    ),
    // The smallest integer.
    ("-9223372036854775808 < x", "-9223372036854775808 < x"),
    // Where a value starts, a minus directly before a number is part of the
    // literal, as the second minus of `a - -5`. Any other minus there is a
    // `Neg`, also the one before `(5)`. A minus after a value subtracts.
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
    // A pattern that holds a `"` cannot be written raw, because a raw text
    // ends at the first `"`. The printer writes it with escapes.
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

/// The reason "expected `wanted` after `after`, found `found`". It suggests
/// no nearest word.
fn expected(wanted: &[&str], found: Found, after: Option<&str>) -> Reason {
    Reason::Expected {
        expected: wanted.iter().map(ToString::to_string).collect(),
        found,
        after: after.map(str::to_owned),
        nearest: None,
    }
}

/// The reason for a token that is no word of the language and stands where
/// `wanted` could stand, after `after`. Like `expected`, it suggests no
/// nearest word.
fn found_token(wanted: &[&str], token: &str, after: Option<&str>) -> Reason {
    expected(wanted, Found::Token(token.to_owned()), after)
}

/// The texts that break the cap on length and the cap on nesting, each with
/// the sentence of its reason. They are built once from the limits, because
/// the rows of `faulty` hold `&'static str`.
static OVER_THE_CAPS: LazyLock<[String; 4]> = LazyLock::new(|| {
    [
        // The text pads a condition with spaces to one character more than
        // the cap.
        format!("{:<width$}", "a > 1", width = MAX_TEXT_LENGTH + 1),
        format!("a rule is at most {MAX_TEXT_LENGTH} characters long"),
        // The text nests `x` in one pair of parentheses more than the cap.
        format!(
            "{}x{}",
            "(".repeat(MAX_NESTING + 1),
            ")".repeat(MAX_NESTING + 1)
        ),
        format!("a rule nests at most {MAX_NESTING} levels deep"),
    ]
});

/// The specification of the errors. Each row holds a faulty text, the bytes
/// its error is about, the reason and its sentence.
#[expect(clippy::too_many_lines, reason = "one row per faulty text")]
fn faulty() -> Vec<(&'static str, Range<usize>, Reason, &'static str)> {
    let foreign = |found, write| Reason::ForeignOperator { found, write };
    // What may follow a whole comparison, as `a > 1`.
    let after_condition = ["`and`", "`or`", "`if`", "the end of the rule"];
    // What may follow an operand that has no operator yet, as `a`.
    let after_operand = [
        "an operator",
        "`and`",
        "`or`",
        "`if`",
        "the end of the rule",
    ];
    let count = |name, takes, given| Reason::WrongArgumentCount { name, takes, given };
    let none_operand = |op: &str| Reason::NoneOperand { op: op.to_owned() };
    let [too_long, too_long_sentence, too_deep, too_deep_sentence] = &*OVER_THE_CAPS;
    vec![
        (
            "",
            0..0,
            Reason::Empty,
            "a rule is empty; write an expression",
        ),
        // An error about a missing token points at the empty range at the end
        // of the text, where the end token stands. The row of
        // `price if price > 0` follows the same rule.
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
        // A `NoneOperand` error covers the operator together with the `None`
        // beside it, here `< None`.
        (
            "x < None",
            2..8,
            none_operand("<"),
            "`<` cannot take `None`; to test for a missing value, write `is None`",
        ),
        (
            "x in None",
            2..9,
            none_operand("in"),
            "`in` cannot take `None`; to test for a missing value, write `is None`",
        ),
        (
            "x not in None",
            2..13,
            none_operand("not in"),
            "`not in` cannot take `None`; to test for a missing value, write `is None`",
        ),
        // With `None` on both sides, the error covers the left `None` and the
        // operator, because the parser tests the left operand first.
        (
            "None >= None",
            0..7,
            none_operand(">="),
            "`>=` cannot take `None`; to test for a missing value, write `is None`",
        ),
        (
            "None + 1",
            0..6,
            none_operand("+"),
            "`+` cannot take `None`; to test for a missing value, write `is None`",
        ),
        (
            "x * None",
            2..8,
            none_operand("*"),
            "`*` cannot take `None`; to test for a missing value, write `is None`",
        ),
        (
            "-None > 1",
            0..5,
            none_operand("-"),
            "`-` cannot take `None`; to test for a missing value, write `is None`",
        ),
        // A chained comparison is refused at its second operator, the second
        // `<`.
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
        // A `TextPrefix` error covers the whole text literal, from the prefix
        // to the closing quote.
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
            found_token(&["a name"], ".", Some(".")),
            "expected a name after `.`, found `.`",
        ),
        (
            "record == 1",
            0..6,
            Reason::BareRecord,
            "`record` is the whole record; name a field, as in `record[\"key\"]`",
        ),
        // After a whole condition the error names the condition it follows,
        // as `a > 1` here. Where no whole expression stands before the
        // place, the error names the token before it.
        (
            "a > 1 b < 2",
            6..7,
            found_token(&after_condition, "b", Some("a > 1")),
            "expected `and`, `or`, `if` or the end of the rule after `a > 1`, found `b`",
        ),
        (
            "price * > 3",
            8..9,
            found_token(&["a value"], ">", Some("*")),
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
        // `abs` is two edits from `all` and `any`. `nearest` in `grammar.rs`
        // allows one edit per four characters of the name, and at least one,
        // so the error lists the functions and makes no guess.
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
        // A `WrongArgumentCount` error covers the argument list with its
        // brackets, here `(a, b)`. The row of `name.upper("x")` follows the
        // same rule.
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
            found_token(&["`in`"], "items", Some("i")),
            "expected `in` after `i`, found `items`",
        ),
        // The error stands at the `)`, where `for` should have come.
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
        // A swap of two neighbours counts as one edit, so `adn` is one edit
        // from `and`.
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
            found_token(&after_operand, "iz", Some("x")),
            "expected an operator, `and`, `or`, `if` or the end of the rule after `x`, found `iz`",
        ),
        // `iss` is one edit from `is` and two edits from `in` and `if`, so `is`
        // is the one nearest word within the limit of one edit. The guess
        // comes from every word that the grammar could have read at this
        // place. `is` is one of them, and the error lists it as "an operator".
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
        // The wrong `)` gets the ordinary "expected" error at its place. The
        // parser reports `NeverClosed` only when the text ends where the
        // closing bracket should stand, and this text goes on after the `)`.
        (
            "[1, 2) == x",
            5..6,
            found_token(
                &["an operator", "`and`", "`or`", "`if`", "`,`", "`]`"],
                ")",
                Some("2"),
            ),
            "expected an operator, `and`, `or`, `if`, `,` or `]` after `2`, found `)`",
        ),
        (
            "x not y",
            6..7,
            found_token(&["`in`"], "y", Some("not")),
            "expected `in` after `not`, found `y`",
        ),
        (
            "x is 5",
            5..6,
            found_token(&["`not`", "`None`"], "5", Some("is")),
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
        // The tokenizer reads `==` first, and the third `=` is the mistake.
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
        // A leading zero before a digit is refused also across underscores,
        // as Python refuses `0_7`.
        ("0_7 > x", 0..3, Reason::MalformedNumber, "malformed number"),
        // `9223372036854775808` is one more than the largest i64. A u64 holds
        // it, so the tokenizer reads it, and the grammar refuses it, because it
        // makes an i64 only with a minus before it.
        (
            "9223372036854775808 > x",
            0..19,
            Reason::IntTooLarge,
            "this number is too large for an integer",
        ),
        // No third operand follows the second `<`, so the error has no two
        // comparisons to join with `and`, and its sentence has no rewrite.
        (
            "a < b <",
            6..7,
            Reason::ChainedComparison { write: None },
            "chained comparisons are not supported",
        ),
        // `\u` takes four hex digits, and `\u12` has two, so the tokenizer
        // refuses it as a bad escape.
        (
            "name == \"\\u12\"",
            9..13,
            Reason::BadEscape {
                found: "\\u12".to_owned(),
            },
            "unknown escape `\\u12` in a text",
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
        // A `KeyOnValue` error covers the whole step, the dot and the key. A
        // `WordAfterDot` error, as for `flows.in`, covers the word alone.
        (
            "\"abc\".x == 1",
            5..7,
            Reason::KeyOnValue,
            "only a field has keys and indexes",
        ),
        // The record is an object, so the first step on `record` must be a
        // key. An index there is refused, and the error covers `record`
        // together with its index.
        (
            "record[0] == 1",
            0..9,
            Reason::BareRecord,
            "`record` is the whole record; name a field, as in `record[\"key\"]`",
        ),
        // `class` alone is a field. The parse fails at the `x` right after it,
        // and because the token before the failure is a Python keyword, the
        // error names that keyword.
        (
            "class x",
            0..5,
            Reason::NotInLanguage {
                word: "class".to_owned(),
            },
            "`class` is not part of the rule language",
        ),
        // The error covers the one character past the cap. The text is
        // ASCII, so that character starts at byte `MAX_TEXT_LENGTH`.
        (
            too_long.as_str(),
            MAX_TEXT_LENGTH..MAX_TEXT_LENGTH + 1,
            Reason::TooLong {
                max: MAX_TEXT_LENGTH,
            },
            too_long_sentence.as_str(),
        ),
        // The error covers the opening parenthesis that is one too many,
        // which stands after `MAX_NESTING` others.
        (
            too_deep.as_str(),
            MAX_NESTING..MAX_NESTING + 1,
            Reason::TooDeep { max: MAX_NESTING },
            too_deep_sentence.as_str(),
        ),
    ]
}

proptest! {
    // With `failure_persistence: None` proptest writes no regression file,
    // so a failure leaves no untracked file in the working tree. A failure
    // prints the reason and the minimal failing input.
    #![proptest_config(ProptestConfig {
        failure_persistence: None,
        .. ProptestConfig::default()
    })]

    /// A canonical tree prints to a text that parses back to the same tree.
    #[test]
    fn a_printed_tree_parses_back_to_itself(tree in canonical_in(&[], 3)) {
        // The whole tree is compared, so a missing pair of parentheses or a
        // key written bare where it needs `record` fails.
        let printed = tree.to_string();
        prop_assert_eq!(parse(&printed), Ok(tree), "{}", printed);
    }

    /// Any text gets a tree or an error, and the error's place is a range of
    /// whole characters inside the text.
    #[test]
    fn any_text_gets_a_tree_or_an_error_inside_it(text in mixed_text()) {
        if let Err(error) = parse(&text) {
            prop_assert!(text.get(error.at.clone()).is_some(), "{:?}", error);
        }
    }
}

#[test]
fn a_valid_text_prints_canonically_and_parses_back() {
    for (text, printed) in VALID {
        let tree = parse(text).unwrap_or_else(|e| panic!("{text:?} is refused: {e}"));
        // The second column holds the text that the printer writes for the
        // tree, so each row pins the printer's output for its text.
        assert_eq!(tree.to_string(), *printed, "{text:?}");
        // The printed text parses to the same tree as the text, so the
        // printed column is itself a valid rule.
        assert_eq!(parse(printed).as_ref(), Ok(&tree), "{text:?}");
    }
}

#[test]
fn a_faulty_text_is_refused_at_its_place_with_its_reason() {
    for (text, at, reason, sentence) in faulty() {
        // A faulty text that parses fails here, with the text as message.
        let error = parse(text).expect_err(text);
        assert_eq!((&error.at, &error.reason), (&at, &reason), "{text:?}");
        // The sentence is the reason's `Display`, so the row shows an
        // author's whole view of the mistake.
        assert_eq!(error.to_string(), sentence, "{text:?}");
    }
}

#[test]
fn the_limits_hold_at_their_edges() {
    // The test builds a rule of 401 conditions and pads it with spaces to
    // `MAX_TEXT_LENGTH` characters. The 401 conditions take 4005 characters,
    // and the spaces fill the rest. The text holds about 1600 tokens, so the
    // first assertion also shows that a long text of many tokens parses
    // within `MAX_PARSE_STEPS`.
    let conditions = format!("a > 1{}", " and a > 1".repeat(400));
    let longest = format!("{conditions:<MAX_TEXT_LENGTH$}");
    // A text at the cap parses, and the text one character longer is
    // refused below. The rows of `faulty` hold only refused texts.
    assert!(parse(&longest).is_ok());
    // One more space is the first character past the cap. The text is
    // ASCII, so each character takes one byte, and that character starts at
    // byte `MAX_TEXT_LENGTH`. The error covers every character past the cap,
    // and here that is the one added space.
    let error = parse(&format!("{longest} ")).expect_err("a text over the cap");
    let max = MAX_TEXT_LENGTH;
    assert_eq!(error.reason, Reason::TooLong { max });
    assert_eq!(error.at, max..max + 1);
    assert_eq!(error.to_string(), "a rule is at most 4096 characters long");

    // `nested` builds `x` inside `levels` pairs of parentheses. Each `(`
    // opens one level.
    let nested = |levels: usize| format!("{}x{}", "(".repeat(levels), ")".repeat(levels));
    assert!(parse(&nested(MAX_NESTING)).is_ok());
    let error = parse(&nested(MAX_NESTING + 1)).expect_err("a text nested over the cap");
    let max = MAX_NESTING;
    assert_eq!(error.reason, Reason::TooDeep { max });
    // The error points at the opening bracket that is one too many. It is
    // the 33rd, at byte 32.
    assert_eq!(error.at, max..max + 1);
    assert_eq!(error.to_string(), "a rule nests at most 32 levels deep");
}

/// The largest tree that `canonical_in` generates at depth 3 prints to a
/// text within the cap on length, and that text parses back to the tree.
#[test]
fn the_largest_canonical_tree_fits_the_text_cap() {
    // The longest key is eight control characters, each printed as an escape
    // of four characters. The field has as many keys as the strategy allows.
    let key = || Segment::Key("\u{1}".repeat(8));
    let field = || Expr::Field((0..=MOST_FIELD_STEPS).map(|_| key()).collect());
    // A conditional is the largest node. It holds three operands, as a list,
    // an `and` and an `or` do, and it adds `if`, `else` and the parentheses
    // around a conditional that is its first branch or its test.
    let conditional = |operand: &dyn Fn() -> Expr| Expr::If {
        test: Box::new(operand()),
        then: Box::new(operand()),
        otherwise: Box::new(operand()),
    };
    // Three levels of conditionals stand above the 27 fields.
    let lowest = || conditional(&field);
    let middle = || conditional(&lowest);
    let tree = conditional(&middle);
    let printed = tree.to_string();
    let length = printed.chars().count();
    assert!(length <= MAX_TEXT_LENGTH, "{length} characters");
    assert_eq!(parse(&printed), Ok(tree));
}

#[test]
fn a_deep_faulty_text_is_refused_for_its_own_mistake() {
    // Each form holds what opens a level, what closes it, and how many
    // levels of nesting one copy of the opening takes. In `not (`, `-(` and
    // `(a if b else ` the `not`, the minus and the branch after `else` each
    // open a level of their own beside the `(`, because the grammar calls
    // itself there.
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
        // `30 / levels` copies of a form make a text that nests 30 levels.
        // The depth 30 is a choice that no measurement backs. It stays under
        // `MAX_NESTING`, so `TooDeep` cannot stand in for the mistake.
        let depth = 30 / levels;
        // The text puts the mistake `x y` at the innermost place.
        let text = format!("{}x y{}", open.repeat(depth), close.repeat(depth));
        // `y` starts after the openings and the two bytes of `x `.
        let mistake = open.len() * depth + 2;
        let error = parse(&text).expect_err("a faulty text");
        // `TooComplex` means that the parse passed `MAX_PARSE_STEPS` token
        // tests. That happens only when two alternatives of a choice read
        // the same bracketed text, which the invariant in
        // `engine/src/text/AGENTS.md` forbids. The next assertion would fail
        // too, and this one names the cause.
        assert_ne!(error.reason, Reason::TooComplex, "{open:?}");
        assert_eq!(error.at, mistake..mistake + 1, "{open:?}");
    }
}
