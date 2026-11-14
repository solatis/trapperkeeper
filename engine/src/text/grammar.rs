//! The grammar of the rule language, written with the kit: one function per
//! level of precedence, from the lowest up. The kinds of expression are
//! ADR-0041.

use std::mem;
use std::ops::Range;

use super::error::{Found, Reason, SyntaxError};
use super::kit::{
    Input, Parser, Refusal, Reply, chain_left, choice, delimited, explain, hidden, label, left,
    many, map, nested, opt, or, require, right, spanned, then, then_after, token, try_map,
};
use super::tokens::{Tok, lex};
use crate::expr::{Expr, Segment, TypeName};

// The words of the language: the twelve of ADR-0041 and the two module
// names. A name token that is one of them is no name.
pub(super) const WORDS: [&str; 14] = [
    "and", "or", "not", "in", "is", "if", "else", "for", "None", "True", "False", "record", "re",
    "math",
];

// The two words that name a module and nothing else.
const MODULES: [&str; 2] = ["re", "math"];

// Python keywords outside the language. They are names here, and they are
// used only to explain a failure right after one.
const PYTHON_ONLY: [&str; 24] = [
    "lambda", "def", "class", "import", "from", "return", "while", "elif", "try", "except",
    "finally", "with", "as", "yield", "global", "nonlocal", "assert", "del", "pass", "break",
    "continue", "raise", "async", "await",
];

/// A value with the bytes of the text it came from.
type Placed<T> = (T, Range<usize>);
/// The builder of a node with two operands.
type Binary = fn(Box<Expr>, Box<Expr>) -> Expr;
/// The builder of a node with one operand.
type Unary = fn(Box<Expr>) -> Expr;
/// The builder of a pattern function from its pattern and its text.
type Pattern = fn(String, Box<Expr>) -> Expr;
/// The builder of a quantifier from its name, its list and its test.
type Quantifier = fn(String, Box<Expr>, Box<Expr>) -> Expr;

/// Parses `text` as a rule. Returns the tree, or the first error with its
/// place.
///
/// # Errors
///
/// The first syntax error in `text`.
pub fn parse(text: &str) -> Result<Expr, SyntaxError> {
    // 1. split the text into tokens
    // 2. run the grammar over the tokens
    // 3. on success, resolve the names
    // 4. on failure, pick the reason

    // 1. `lex` does not fail. A text it cannot split ends in a `Bad` token,
    //    as the `;` of `amount > 100;`, and the grammar meets it there.
    let input = Input::new(text, lex(text));
    // 2. `rule` reads from the first token. It gives a tree, no match, or a
    //    refusal such as `UnknownFunction` for `lenn(name) > 3`.
    let refusal = match rule(&input, 0) {
        Ok(Some((mut tree, _))) => {
            // 3. The grammar holds every bare name as an element, because
            //    in `any(test for name in list)` the name comes after the
            //    test that uses it.
            resolve(&mut tree, &mut Vec::new());
            return Ok(tree);
        }
        // 2. No match leaves the log of the token tests as the only source
        //    of the error, as for `a > 1 b`.
        Ok(None) => None,
        Err(refusal) => Some(refusal),
    };
    // 4. `explain` builds the error from the refusal and the log. A refusal
    //    wins unless a note lies further in the text.
    let error = explain(&input, refusal);
    // 4. A refusal or a tokenizer's reason already says what is wrong, so it
    //    is returned as it is. `0 < amount < 10` returns here.
    if !matches!(error.reason, Reason::Expected { .. }) {
        return Err(error);
    }
    // 4. Two targeted reasons are tried before the generic one, and neither
    //    changes which texts are valid: a name that is a word in the wrong
    //    case, as `AND` in `a > 1 AND b < 2`, and a Python keyword right
    //    before the failure, as `lambda` in `lambda x: x > 1`.
    let targeted = miscased_word(&input).or_else(|| python_only_word(&input));
    // 4. Without a targeted reason, the generic error gets what the grammar
    //    knows of the name at its place, as "did you mean `and`?" for `adn`.
    Err(targeted.unwrap_or_else(|| with_words(&input, error)))
}

/// `KeywordCase`, when the token at the failure is a name that differs only
/// in case from a word of the language that could stand there.
fn miscased_word(i: &Input<'_>) -> Option<SyntaxError> {
    // `wanted` also holds the notes that `label` and `hidden` keep out of
    // the error. In `x NOT in y` it holds `not` behind "an operator".
    let (pos, wanted) = i.furthest();
    let token = i.tok(pos);
    // Only a name token can be a miscased word: `AND` in `a > 1 AND b < 2`.
    let Tok::Name(found) = &token.kind else {
        return None;
    };
    // `find` passes a reference to an item of `WORDS.iter()`, hence the
    // `&&&str`. A name in the same case as the word is no mistake of case.
    let miscased = |w: &&&str| w.eq_ignore_ascii_case(found) && **w != found.as_str();
    // No two words differ only in case, so at most one word matches. `AND`
    // finds `and`, and `none` in `x is none` finds `None`.
    let keyword = *WORDS.iter().find(miscased)?;
    let reason = Reason::KeywordCase {
        found: found.clone(),
        keyword,
    };
    // The error covers the name, bytes 6 to 9 of `a > 1 AND b < 2`.
    let at = token.at.clone();
    // The word must be one that a test wanted at this place. A test notes a
    // word in backticks, as `word` names it. In `a > 1 Record b` no test
    // wanted `record`, so the error stays generic.
    wanted
        .contains(&format!("`{keyword}`"))
        .then_some(SyntaxError { at, reason })
}

/// `NotInLanguage`, when the token before the failure is a Python keyword
/// outside the language. The error points at that keyword.
fn python_only_word(i: &Input<'_>) -> Option<SyntaxError> {
    // The parse failed at the furthest note: `x` in `lambda x: x > 1`, where
    // `lambda` was read as a field and `x` cannot follow it.
    let (pos, _) = i.furthest();
    // A failure at the first token has no token before it, as in `in > 5`.
    let token = i.tok(pos.checked_sub(1)?);
    // A Python keyword is a name token, because the language reserves only
    // its own words (ADR-0041).
    let Tok::Name(word) = &token.kind else {
        return None;
    };
    let reason = Reason::NotInLanguage { word: word.clone() };
    // The error covers the keyword, bytes 0 to 6 of `lambda x: x > 1`.
    let at = token.at.clone();
    // Any other name before the failure, as `a` in `a adn b`, gives none.
    PYTHON_ONLY
        .contains(&word.as_str())
        .then_some(SyntaxError { at, reason })
}

/// The generic error with what the grammar knows of the name at its place: a
/// word of the language that stands there, or the nearest word that could
/// stand there.
fn with_words(i: &Input<'_>, error: SyntaxError) -> SyntaxError {
    let (pos, wanted) = i.furthest();
    // Only a name token at the place gets help, and `parse` passes only an
    // `Expected` error. The pattern takes `expected` and `after` from it to
    // build the new reason. `a > )` returns the error as it is.
    let (
        Tok::Name(name),
        Reason::Expected {
            expected, after, ..
        },
    ) = (&i.tok(pos).kind, &error.reason)
    else {
        return error;
    };
    // The name may be a word of the language, as `or` in `a and or b`.
    let word = WORDS.iter().find(|w| **w == name).copied();
    // Only a word that some test wanted at this place is offered. `wanted`
    // holds the notes behind a label too. After the `x` of `x iss None` it
    // holds `is`, which the label "an operator" keeps out of the error.
    let could_stand = |w: &&&str| wanted.contains(&format!("`{w}`"));
    let known: Vec<&str> = WORDS.iter().filter(could_stand).copied().collect();
    let reason = Reason::Expected {
        expected: expected.clone(),
        // A word of the language is `Found::Word`, whose sentence says to
        // write `record["or"]` for a field `or`.
        found: word.map_or_else(|| Found::Token(name.clone()), Found::Word),
        after: after.clone(),
        // A word of the language is spelled right and gets no guess. `adn`
        // in `a adn b` gets `and`.
        nearest: nearest(name, &known).filter(|_| word.is_none()),
    };
    SyntaxError {
        at: error.at,
        reason,
    }
}

// Tokens.

/// The word `w` of the language, with its place.
fn word(w: &'static str) -> impl Parser<Range<usize>> {
    let is_word = move |t: &super::tokens::Token| {
        matches!(&t.kind, Tok::Name(n) if n == w).then(|| t.at.clone())
    };
    // The note is the word in backticks, as in "expected `in` after `not`".
    // `miscased_word` and `with_words` look words up in this form.
    token(format!("`{w}`"), is_word)
}

/// The operator or punctuation `s`, with its place.
fn punct(s: &'static str) -> impl Parser<Range<usize>> {
    let is_punct = move |t: &super::tokens::Token| {
        matches!(t.kind, Tok::Punct(x) if x == s).then(|| t.at.clone())
    };
    token(format!("`{s}`"), is_punct)
}

/// One operator of `table`, as the builder of its node.
fn symbol(table: &'static [(&'static str, Binary)]) -> impl Parser<Binary> {
    // The note is "an operator" whichever operator was wanted. The `>` of
    // `amount > 100` gives `Expr::Gt` from `COMPARISONS`.
    token("an operator", move |t| match t.kind {
        Tok::Punct(s) => table.iter().find(|(o, _)| *o == s).map(|(_, make)| *make),
        _ => None,
    })
}

/// A name that is no word of the language.
fn name(i: &Input<'_>, p: usize) -> Reply<String> {
    token("a name", |t| match &t.kind {
        Tok::Name(n) if !WORDS.contains(&n.as_str()) => Some(n.clone()),
        _ => None,
    })(i, p)
}

/// Any name token, also a word of the language.
fn any_name(i: &Input<'_>, p: usize) -> Reply<String> {
    token("a name", |t| match &t.kind {
        Tok::Name(n) => Some(n.clone()),
        _ => None,
    })(i, p)
}

// Expressions, from the lowest level of precedence up.

/// `rule := expr end`
///
/// Reads a whole rule, such as `amount > 100`, up to the end of the text.
fn rule(i: &Input<'_>, p: usize) -> Reply<Expr> {
    // The empty text gets a reason of its own, over the whole text.
    if matches!(i.tok(p).kind, Tok::End) {
        let (at, reason) = (0..i.text.len(), Reason::Empty);
        let error = SyntaxError { at, reason };
        return Err(Refusal { pos: p, error });
    }
    // An error at the end says after which expression it stands, as in
    // "after `a > 1`" for `a > 1 b`. The map drops the `()` of the end.
    map(then_after(expr, end_of_rule), |(e, ())| e)(i, p)
}

/// `end := <the end of the text>`
///
/// Reads the end of `amount > 100`. In `a > 1) and b` it refuses the `)`.
fn end_of_rule(i: &Input<'_>, p: usize) -> Reply<()> {
    // `end` reads the `End` token, or notes "the end of the rule".
    let end = token("the end of the rule", |t| {
        matches!(t.kind, Tok::End).then_some(())
    });
    // `closer` reads a `)` or a `]` and gives its character. The note has no
    // name, because `hidden` below keeps it out of every error.
    let closer = token("", |t| match t.kind {
        Tok::Punct(s @ (")" | "]")) => s.chars().next(),
        _ => None,
    });
    // A closing bracket here closes nothing, because every opening bracket
    // consumes its own. The refusal covers the `)`, bytes 5 to 6 of
    // `a > 1) and b`.
    let stray = try_map(closer, |_, close, at| {
        let reason = Reason::ClosesNothing { close };
        Err(SyntaxError { at, reason })
    });
    // The rule tries the end of the text first, then a stray bracket. Without
    // `hidden`, the error for `a > 1 b` would list the empty name of
    // `closer`.
    or(end, hidden(stray))(i, p)
}

/// `expr := conditional`
///
/// Reads any expression, such as `a or b`. It is what stands in parentheses,
/// in a list and in an argument.
fn expr(i: &Input<'_>, p: usize) -> Reply<Expr> {
    // An error says "an expression" and does not list every token that can
    // start one: `in > 5` gets "expected an expression, found `in`".
    label("an expression", conditional)(i, p)
}

/// `conditional := or_expr ("if" or_expr "else" conditional)?`
///
/// Reads `price if price > 0 else 0`. The branch after `else` may be a
/// conditional again, so `a if b else c if d else e` groups to the right.
fn conditional(i: &Input<'_>, p: usize) -> Reply<Expr> {
    // Python's `a if b` without `else` is the mistake to name. `price if
    // price > 0` is refused at the end of the text, where `else` is missing.
    let needs_else = require(Reason::ConditionalNeedsElse, word("else"));
    // `tail` reads `if price > 0 else 0` and gives the test and the branch
    // after `else`. The test is read at the level of `or`, so a conditional
    // in it needs parentheses. The branch after `else` is nested, because
    // this rule calls itself there.
    let tail = then(right(word("if"), or_expr), nested(needs_else, conditional));
    // The closure's `then` is the value before `if`, `price` in the example.
    // It shadows the combinator `then` inside the closure.
    map(then(or_expr, opt(tail)), |(then, tail)| match tail {
        // `a or b`, with no `if` after it.
        None => then,
        // `then if test else otherwise`.
        Some((test, otherwise)) => Expr::If {
            test: Box::new(test),
            then: Box::new(then),
            otherwise: Box::new(otherwise),
        },
    })(i, p)
}

/// `or_expr := and_expr ("or" and_expr)*`
///
/// Reads `a or b or c` as one `Or` of three operands.
fn or_expr(i: &Input<'_>, p: usize) -> Reply<Expr> {
    // `chain_left` folds `a or b or c` as `join(join(a, b), c)`, and `join`
    // flattens that into one list. The `_` is the place of `or`.
    chain_left(and_expr, word("or"), |l, _, r| join(Expr::Or, l, r))(i, p)
}

/// `and_expr := not_expr ("and" not_expr)*`
///
/// Reads `a > 1 and b < 2 and c` as one `And` of three operands.
fn and_expr(i: &Input<'_>, p: usize) -> Reply<Expr> {
    // The same fold as in `or_expr`, with `and`.
    chain_left(not_expr, word("and"), |l, _, r| join(Expr::And, l, r))(i, p)
}

/// Joins two operands of `and` or `or` into one node, and splices in an
/// operand that is a node of the same kind, so `(a and b) and c` is one list
/// of three.
fn join(make: fn(Vec<Expr>) -> Expr, l: Expr, r: Expr) -> Expr {
    // The kind of node that `make` builds, `And` or `Or`. The empty node is
    // built only to learn its kind.
    let kind = mem::discriminant(&make(Vec::new()));
    // `operands` gives the operands that one side brings into the list.
    let operands = |e: Expr| {
        let same = mem::discriminant(&e) == kind;
        match e {
            // A node of the same kind brings its own operands: the left side
            // `a and b` of `(a and b) and c` brings `a` and `b`.
            Expr::And(operands) | Expr::Or(operands) if same => operands,
            // Anything else is one operand. That includes a node of the other
            // kind, as `(c or d)` in `a and (c or d)`.
            e => vec![e],
        }
    };
    // In `(a and b) and c` the left side gives `a, b` and the right side `c`.
    let mut all = operands(l);
    all.extend(operands(r));
    make(all)
}

/// `not_expr := "not" not_expr | comparison`
///
/// Reads `not active` and `not not x`. A `not` that follows a value is not
/// read here: it starts `not in`, which `comparison` reads.
fn not_expr(i: &Input<'_>, p: usize) -> Reply<Expr> {
    let not_or_comparison = or(
        // `not x`. The operand is read by this same rule, so `not not x`
        // nests, and each `not` counts as a level of nesting.
        map(nested(word("not"), not_expr), |e| Expr::Not(Box::new(e))),
        // Anything else is a comparison or a form that binds tighter.
        comparison,
    );
    // An error says "a value", and `not` is not listed beside it: `a and or
    // b` gets "expected a value after `and`, found `or`".
    label("a value", not_or_comparison)(i, p)
}

// The comparison operators and the builder of each one's node.
const COMPARISONS: [(&str, Binary); 6] = [
    ("==", Expr::Eq),
    ("!=", Expr::Ne),
    ("<=", Expr::Le),
    (">=", Expr::Ge),
    ("<", Expr::Lt),
    (">", Expr::Gt),
];

/// `compare_op := "==" | "!=" | "<=" | ">=" | "<" | ">" | "in" | "not" "in"`
///
/// Reads the operator of `x not in (1, 2)` and gives the builder of its
/// node.
fn compare_op(i: &Input<'_>, p: usize) -> Reply<Binary> {
    // The `as Binary` casts turn each constructor into the function pointer
    // that `symbol` gives, so the three alternatives have one type.
    choice!(
        // `==`, `!=`, `<=`, `>=`, `<` and `>`.
        symbol(&COMPARISONS),
        // `x in y`.
        map(word("in"), |_| Expr::In as Binary),
        // `x not in y`. A `not` after an operand can only start `not in`, so
        // `x not y` gets "expected `in` after `not`, found `y`".
        map(then(word("not"), word("in")), |_| Expr::NotIn as Binary),
    )(i, p)
}

/// What follows the left operand of a comparison.
enum Tail {
    // The builder of the node and the place of its operator, then the right
    // operand and its place.
    Binary(Binary, Range<usize>, Expr, Range<usize>),
    // `is None`, or `is not None` when the flag is set.
    IsNone(bool),
}

/// `compare_tail := compare_op additive | "is" "not"? "None"`
///
/// Reads the `> 100` of `amount > 100` and the `is not None` of
/// `discount is not None`.
fn compare_tail(i: &Input<'_>, p: usize) -> Reply<Tail> {
    // `> 100` gives the operator and the right operand, each with its
    // bytes. The places are kept for the errors of `build_comparison`. The
    // operand is read by `additive`, so in `0 < amount < 10` it stops before
    // the second `<`.
    let operator_and_operand = then(spanned(compare_op), spanned(additive));
    // In `x == None` the parts are `Expr::Eq` with bytes 2 to 4, then the
    // literal `None` with bytes 5 to 9.
    let binary = map(operator_and_operand, |((make, op_at), (r, r_at))| {
        Tail::Binary(make, op_at, r, r_at)
    });
    // `is None` and `is not None`. Only `None` may follow `is`, so `x is 5`
    // gets "expected `not` or `None` after `is`".
    let is_none = right(word("is"), then(opt(word("not")), word("None")));
    // `right` dropped the place of `is`. The value is the optional place of
    // `not` and the place of `None`, and only whether `not` stood counts.
    let is_none = map(is_none, |(not, _)| Tail::IsNone(not.is_some()));
    // An error says "an operator" once. Without the label, the error for
    // `a adn b` would also list `in`, `not` and `is`.
    label("an operator", or(binary, is_none))(i, p)
}

/// A second operator after a comparison with its place, and the operand
/// after it with its place, when there is one.
type Chained = Option<(Placed<()>, Option<Placed<Expr>>)>;

/// `comparison := additive compare_tail?`
///
/// Reads `amount > 100`, `x not in (1, 2)` and `discount is not None`, or
/// the operand alone. Comparisons do not chain, so `0 < amount < 10` is
/// refused.
fn comparison(i: &Input<'_>, p: usize) -> Reply<Expr> {
    // `again` reads a second operator, as the second `<` of `0 < amount <
    // 10` or the second `is` of `x is None is None`, and gives `()`.
    // It is read only to refuse the chain with the rewrite that names its
    // operands. It is hidden, or the error for `a > 1 b` would list "an
    // operator", `in`, `not` and `is`.
    let again = hidden(or(map(compare_op, |_| ()), map(word("is"), |_| ())));
    // `chained` gives the second operator with its place, then the third
    // operand with its place: `10` in `0 < amount < 10`, nothing in `a < b <`.
    let chained = opt(then(spanned(again), opt(spanned(additive))));
    // `tail` reads what follows the first operand: `< amount` and the chain
    // after it. In `amount and x` nothing follows.
    let tail = opt(then(compare_tail, chained));
    // The rule reads the first operand with its place, then the tail.
    // `build_comparison` builds the node from them or refuses the text.
    try_map(then(spanned(additive), tail), build_comparison)(i, p)
}

/// The node of a comparison from its parts, or why it is none: a chain of
/// comparisons, or a comparison with the literal `None`.
fn build_comparison(
    i: &Input<'_>,
    ((left, left_at), tail): (Placed<Expr>, Option<(Tail, Chained)>),
    _: Range<usize>,
) -> Result<Expr, SyntaxError> {
    // `left` and `left_at` are the first operand and its bytes, `0` and 0 to
    // 1 in `0 < amount < 10`. The last parameter is the bytes of the whole
    // comparison, which `try_map` passes and no error here needs.
    let text = |at: &Range<usize>| i.slice(at);
    // No operator followed: `amount` in `amount and x` is the operand alone.
    let Some((tail, chained)) = tail else {
        return Ok(left);
    };
    // A second operator stood after the comparison. `again_at` is its
    // bytes, 11 to 12 of `0 < amount < 10`, and `third` is `10` with its
    // bytes.
    if let Some((((), again_at), third)) = chained {
        // The rewrite needs two comparisons of two operands each, so it is
        // left out after `is None` and when the third operand is missing.
        let write = match (&tail, third) {
            (Tail::Binary(_, op_at, _, right_at), Some((_, third_at))) => {
                // `first` is `0 < amount`, written from the three parts.
                let first = format!("{} {} {}", text(&left_at), text(op_at), text(right_at));
                // `amount < 10`: the middle operand is written twice.
                let second = format!("{} {} {}", text(right_at), text(&again_at), text(&third_at));
                Some(format!("{first} and {second}"))
            }
            // `a < b <` and `x is None is None` get no rewrite.
            _ => None,
        };
        let reason = Reason::ChainedComparison { write };
        // The error covers the second operator.
        return Err(SyntaxError {
            at: again_at,
            reason,
        });
    }
    // `is None` and `is not None` are complete nodes. A binary tail gives
    // its parts for the check of `None` below.
    let (make, op_at, right, right_at) = match tail {
        Tail::IsNone(false) => return Ok(Expr::IsNone(Box::new(left))),
        Tail::IsNone(true) => return Ok(Expr::IsNotNone(Box::new(left))),
        Tail::Binary(make, op_at, right, right_at) => (make, op_at, right, right_at),
    };
    // A comparison with a missing value turns the rule off (ADR-0005), so
    // `x == None` could never do what its author meant. The refusal gives
    // the `is None` form of the same test, with `None` on either side.
    // `none_test` is the operator and the form that replaces it.
    let none_test = match text(&op_at) {
        "==" => Some(("==", "is None")),
        "!=" => Some(("!=", "is not None")),
        // `x < None` is built as it is.
        _ => None,
    };
    // Each flag says whether its operand is the literal `None`.
    let (left_none, right_none) = (matches!(left, Expr::None), matches!(right, Expr::None));
    if let Some((op, is)) = none_test.filter(|_| left_none || right_none) {
        // The error covers the operator and the `None` beside it, and the
        // other operand is what the rewrite tests.
        let (at, operand) = if right_none {
            // `x == None`: bytes 2 to 9, `== None`, and the operand `x`.
            (op_at.start..right_at.end, &left_at)
        } else {
            // `None == x`: bytes 0 to 7, `None ==`, and the operand `x`.
            (left_at.start..op_at.end, &right_at)
        };
        // `write` is `x is None`, or `x is not None` for `x != None`.
        let write = format!("{} {is}", text(operand));
        let reason = Reason::NoneComparison { op, write };
        return Err(SyntaxError { at, reason });
    }
    Ok(make(Box::new(left), Box::new(right)))
}

// The arithmetic operators of each level and the builder of each one's node.
// They are hidden from expected sets, because they may follow almost any
// value and would bury the keywords.
const ADDITIVE: [(&str, Binary); 2] = [("+", Expr::Add), ("-", Expr::Sub)];
const TERM: [(&str, Binary); 3] = [("*", Expr::Mul), ("/", Expr::Div), ("%", Expr::Mod)];

/// `additive := term (("+" | "-") term)*`
///
/// Reads `a - b + 1`, grouped to the left as `(a - b) + 1`.
fn additive(i: &Input<'_>, p: usize) -> Reply<Expr> {
    let op = hidden(symbol(&ADDITIVE));
    // `make` is the builder that `symbol` found for the operator, as
    // `Expr::Sub` for the `-` of `a - b`.
    chain_left(term, op, |l, make, r| make(Box::new(l), Box::new(r)))(i, p)
}

/// `term := unary (("*" | "/" | "%") unary)*`
///
/// Reads `price * qty % 7`, grouped to the left as `(price * qty) % 7`.
fn term(i: &Input<'_>, p: usize) -> Reply<Expr> {
    let op = hidden(symbol(&TERM));
    // The same fold as in `additive`, one level tighter.
    chain_left(unary, op, |l, make, r| make(Box::new(l), Box::new(r)))(i, p)
}

/// `unary := "-" number | "-" unary | postfix`
///
/// Reads a leading minus. On a number token it gives a negative literal, as
/// in `-5`, and on anything else a `Neg`, as in `-a` and `-(5)`.
fn unary(i: &Input<'_>, p: usize) -> Reply<Expr> {
    // `int` gives the magnitude of an integer token. The tokenizer reads a
    // number without its sign.
    let int = token("a number", |t| match t.kind {
        Tok::Int(v) => Some(v),
        _ => None,
    });
    let float = token("a number", |t| match t.kind {
        Tok::Float(v) => Some(v),
        _ => None,
    });
    let minus_or_postfix = choice!(
        // `-5`. The minus folds only into a number token and never into a
        // parsed operand, so `-(5)` stays a `Neg` and a printed tree parses
        // back to itself. The magnitude may be 2^63 with the minus, so the
        // smallest integer can be written. The number is hidden, or `- )`
        // would get "expected a number or a value".
        try_map(right(punct("-"), hidden(int)), |_, v, at| {
            let reason = Reason::IntTooLarge;
            // 0 minus the magnitude fails above 2^63. The error then covers
            // the minus and the number, bytes 0 to 20 of
            // `-9223372036854775809 < x`.
            let negative = 0_i64.checked_sub_unsigned(v);
            negative.map(Expr::Int).ok_or(SyntaxError { at, reason })
        }),
        // `-0.5`.
        map(right(punct("-"), hidden(float)), |v| Expr::Float(-v)),
        // `-a` and `- -5`. Each minus counts as a level of nesting.
        map(nested(punct("-"), unary), |e| Expr::Neg(Box::new(e))),
        // Anything else binds tighter than a minus.
        postfix,
    );
    // An error says "a value" and does not list every token that can start
    // one.
    label("a value", minus_or_postfix)(i, p)
}

// Postfix: keys, indexes and method calls.

/// One step after an atom, as `postfix` reads it.
enum Post {
    // `.key`, `[0]` and `["key"]`.
    Step(Segment),
    // A word of the language after a dot, with its place.
    Word(String, Range<usize>),
    // `.name(arguments)`: the name and the arguments, each with its place.
    Call(String, Range<usize>, Vec<Expr>, Range<usize>),
}

/// `postfix := atom ("." name | "[" index "]" | "[" text "]" | "." name "(" items ")")*`
///
/// Reads `items[0].price`, `flows["in"]` and `name.strip().upper()`.
fn postfix(i: &Input<'_>, p: usize) -> Reply<Expr> {
    // `steps` gives each step with its bytes: `[0]`, then `.price`, in
    // `items[0].price`. The steps are hidden from expected sets, because a
    // `.` or a `[` may follow almost any value. A step that fails after its
    // first token still reports, as `customer..country` with "expected a
    // name".
    let steps = many(spanned(hidden(or(dot_step, bracket_step))));
    // The rule reads the atom with its bytes, then the steps.
    // `apply_postfixes` builds the node or refuses the text.
    try_map(then(spanned(atom), steps), apply_postfixes)(i, p)
}

/// `"." name ("(" items ")")?`
///
/// Reads the `.price` of `item.price` and the `.upper()` of `name.upper()`.
fn dot_step(i: &Input<'_>, p: usize) -> Reply<Post> {
    // Any name token is read here, also a word of the language, so that
    // `flows.in` gets a reason of its own in `apply_postfixes`. The name
    // comes with its bytes, then the arguments with theirs when a `(`
    // follows.
    let step = then(spanned(any_name), opt(spanned(args)));
    map(right(punct("."), step), |((name, at), call)| match call {
        // `.upper()`: a method call.
        Some((args, args_at)) => Post::Call(name, at, args, args_at),
        // `.in`: a word of the language is never a key.
        None if WORDS.contains(&name.as_str()) => Post::Word(name, at),
        // `.price`: a key.
        None => Post::Step(Segment::Key(name)),
    })(i, p)
}

/// `"[" (index | text) "]"`
///
/// Reads the `[0]` of `items[0]` and the `["in"]` of `flows["in"]`.
fn bracket_step(i: &Input<'_>, p: usize) -> Reply<Post> {
    // `step` reads the `0` of `items[0]` or the `"in"` of `flows["in"]`. An
    // index above `u32::MAX` does not match, so `items[99999999999]` gets
    // "expected an index or a key".
    let step = token("an index or a key", |t| match &t.kind {
        Tok::Int(v) => u32::try_from(*v).ok().map(Segment::Index),
        Tok::Text(s) => Some(Segment::Key(s.clone())),
        _ => None,
    });
    // `number` reads any integer token, as the `1` of `[-1]`.
    let number = token("a number", |t| matches!(t.kind, Tok::Int(_)).then_some(()));
    // `[-1]`. Python's index from the end is refused by name (ADR-0041). The
    // refusal covers `-1`, bytes 6 to 8 of `items[-1].price > 5`. The
    // closure always refuses, and the turbofish names `Segment` as the value
    // it would give, so both alternatives below give a `Segment`.
    let negative = try_map(then(punct("-"), number), |_, _, at| {
        let reason = Reason::NegativeIndex;
        Err::<Segment, _>(SyntaxError { at, reason })
    });
    // `inside` reads the index or the key, then the `]`. The negative index
    // is hidden, or `items[x]` would get "expected an index or a key or
    // `-`".
    let inside = left(or(step, hidden(negative)), punct("]"));
    map(right(punct("["), inside), Post::Step)(i, p)
}

/// Applies the postfix steps to the atom, left to right.
fn apply_postfixes(
    i: &Input<'_>,
    ((atom, atom_at), posts): (Placed<Expr>, Vec<Placed<Post>>),
    _: Range<usize>,
) -> Result<Expr, SyntaxError> {
    // `atom` and `atom_at` are the atom and its bytes, `items` and 0 to 5
    // in `items[0].price`. `posts` holds each step with its bytes. The last
    // parameter is the bytes of the whole form, which no error here needs.
    let refuse = |at: Range<usize>, reason| Err(SyntaxError { at, reason });
    // `record` alone is a field with an empty path.
    let bare_record = |e: &Expr| matches!(e, Expr::Field(path) if path.is_empty());
    // `e` holds the expression so far: `items`, then `items[0]`, then
    // `items[0].price`.
    let mut e = atom;
    // `at` is the bytes of one step, as the `[0]` of `items[0].price`.
    for (post, at) in posts {
        e = match (post, e) {
            // `record[0]`: the record is an object, so its first step is a
            // key. The error covers `record[0]`, bytes 0 to 9.
            (Post::Step(Segment::Index(_)), e) if bare_record(&e) => {
                return refuse(atom_at.start..at.end, Reason::BareRecord);
            }
            // A step on a field: `.country` on `customer`, or the first key
            // of `record["unit price"]`.
            (Post::Step(step), Expr::Field(mut path)) => {
                path.push(step);
                Expr::Field(path)
            }
            // A step on a bare name, as `.price` on `i` in `any(i.price > 100
            // for i in items)`. `resolve` later decides what `i` is.
            (Post::Step(step), Expr::Element { name, mut path }) => {
                path.push(step);
                Expr::Element { name, path }
            }
            // Only a path has keys. `"abc".x == 1` is refused at `.x`,
            // bytes 5 to 7.
            (Post::Step(_), _) => return refuse(at, Reason::KeyOnValue),
            // `flows.in`, refused at `in`, bytes 6 to 8.
            (Post::Word(word, word_at), _) => {
                // The bracket form of the same key, built from the text
                // before the dot: `flows`, then `flows["in"]`. `trim_end`
                // drops the space of `flows .in`.
                let before = i.slice(&(atom_at.start..at.start)).trim_end();
                let write = format!("{before}[\"{word}\"]");
                return refuse(word_at, Reason::WordAfterDot { word, write });
            }
            // `record.upper()`: a method on the whole record, refused at
            // `record`, bytes 0 to 6.
            (Post::Call(..), e) if bare_record(&e) => {
                return refuse(atom_at, Reason::BareRecord);
            }
            // `name.upper()`: the method wraps the expression so far. In
            // `name.strip().upper()`, `strip` wraps `name` and `upper` wraps
            // that.
            (Post::Call(name, name_at, args, args_at), e) => {
                method(e, &name, name_at, args, args_at)?
            }
        };
    }
    // `record` with no key is the whole record, which is no value. `record
    // == 1` is refused at `record`, bytes 0 to 6.
    if bare_record(&e) {
        return refuse(atom_at, Reason::BareRecord);
    }
    Ok(e)
}

/// How many arguments a method takes, with the builder of its node.
#[derive(Clone, Copy)]
pub(super) enum Arity {
    Zero(Unary),
    One(Binary),
}

// The methods of the closed list of ADR-0041: the name an author types, and
// the builder.
pub(super) const METHODS: [(&str, Arity); 14] = [
    ("startswith", Arity::One(Expr::StartsWith)),
    ("endswith", Arity::One(Expr::EndsWith)),
    ("upper", Arity::Zero(Expr::Upper)),
    ("lower", Arity::Zero(Expr::Lower)),
    ("strip", Arity::Zero(Expr::Strip)),
    ("lstrip", Arity::Zero(Expr::LStrip)),
    ("rstrip", Arity::Zero(Expr::RStrip)),
    ("isdigit", Arity::Zero(Expr::IsDigit)),
    ("isalpha", Arity::Zero(Expr::IsAlpha)),
    ("isalnum", Arity::Zero(Expr::IsAlnum)),
    ("isspace", Arity::Zero(Expr::IsSpace)),
    ("isupper", Arity::Zero(Expr::IsUpper)),
    ("islower", Arity::Zero(Expr::IsLower)),
    ("isascii", Arity::Zero(Expr::IsAscii)),
];

/// A method call on `receiver`, or why it is none: an unknown method with
/// the nearest known one, or a wrong number of arguments.
fn method(
    receiver: Expr,
    name: &str,
    name_at: Range<usize>,
    mut args: Vec<Expr>,
    args_at: Range<usize>,
) -> Result<Expr, SyntaxError> {
    // The table's `name` shadows the argument, because the reason
    // `WrongArgumentCount` holds a `&'static str`.
    let Some(&(name, arity)) = METHODS.iter().find(|(n, _)| *n == name) else {
        // `name.startwith("a")` is refused at `startwith`, bytes 5 to 14,
        // with the nearest method `startswith`.
        let nearest = nearest(name, &names(&METHODS));
        let name = name.to_owned();
        let reason = Reason::UnknownMethod { name, nearest };
        return Err(SyntaxError {
            at: name_at,
            reason,
        });
    };
    let receiver = Box::new(receiver);
    // `takes` is 0 or 1, and `given` counts the arguments in the text.
    let (takes, given) = (usize::from(matches!(arity, Arity::One(_))), args.len());
    // `pop` takes the last argument off.
    match (arity, args.pop()) {
        // `name.upper()`.
        (Arity::Zero(make), None) => Ok(make(receiver)),
        // `name.startswith("a")`, with one argument and no more.
        (Arity::One(make), Some(argument)) if given == 1 => Ok(make(receiver, Box::new(argument))),
        // `name.upper("x")` is refused at `("x")`, bytes 10 to 15, and so is
        // `name.startswith()` at `()`.
        _ => {
            let reason = Reason::WrongArgumentCount { name, takes, given };
            Err(SyntaxError {
                at: args_at,
                reason,
            })
        }
    }
}

/// The names of a table, as an error lists them and as `nearest` reads them.
pub(super) fn names<T>(table: &[(&'static str, T)]) -> Vec<&'static str> {
    table.iter().map(|(name, _)| *name).collect()
}

/// The known name nearest to `word`, when it is near enough to be a typo:
/// one edit per four characters, and at least one. Two names that are
/// equally near give none, because neither is the better guess.
fn nearest<'a>(word: &str, known: &[&'a str]) -> Option<&'a str> {
    // `lenn` may be one edit off and `startwith` two.
    let limit = (word.chars().count() / 4).max(1);
    // `distances` pairs each known name with its distance from `word`.
    let distances: Vec<(usize, &str)> =
        known.iter().map(|k| (edit_distance(word, k), *k)).collect();
    // `min_by_key` gives the least distance and the first name at it. An
    // empty list of known names gives none.
    let (least, name) = *distances.iter().min_by_key(|(d, _)| *d)?;
    // `iz` is one edit from `is`, `in` and `if`, so it gets no guess.
    let unique = distances.iter().filter(|(d, _)| *d == least).count() == 1;
    // `abs` is two edits from `all` and `any`, over its limit of one, so
    // `abs(a - b)` gets the whole list of functions.
    (least <= limit && unique).then_some(name)
}

/// How many edits turn `a` into `b`. An edit inserts, deletes or replaces a
/// character, or swaps two neighbours.
fn edit_distance(a: &str, b: &str) -> usize {
    // The words become lists of characters, so a character of several bytes
    // counts as one.
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    // `rows[i][j]` is the number of edits that turn the first `i` characters
    // of `a` into the first `j` characters of `b`.
    let mut rows = vec![vec![0; b.len() + 1]; a.len() + 1];
    for i in 0..=a.len() {
        for j in 0..=b.len() {
            // From or to the empty prefix, every character is one insertion
            // or one deletion: row 0 holds `j`, and column 0 holds `i`.
            if i == 0 || j == 0 {
                rows[i][j] = i + j;
                continue;
            }
            // Replace the last character of the prefix of `a` by the last
            // character of the prefix of `b`. That costs nothing when the two
            // are equal.
            let replaced = rows[i - 1][j - 1] + usize::from(a[i - 1] != b[j - 1]);
            // Or delete the last character of `a`, or insert the last
            // character of `b`, whichever needs the fewest edits in all.
            let mut least = replaced.min(rows[i - 1][j] + 1).min(rows[i][j - 1] + 1);
            // Or swap: the last two characters of `a` are the last two of
            // `b` in the other order. A swap counts as one edit, so `adn` is
            // one edit from `and`.
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                least = least.min(rows[i - 2][j - 2] + 1);
            }
            rows[i][j] = least;
        }
    }
    // The last cell holds the distance of the whole words.
    rows[a.len()][b.len()]
}

// Atoms.

/// `atom := call | "record" | name | literal | "(" items ")" | "[" items "]"`
///
/// Reads `len(name)`, `record`, `amount`, `100`, `(a or b)` and `[1, 2]`.
fn atom(i: &Input<'_>, p: usize) -> Reply<Expr> {
    choice!(
        // `len(name)`. The calls come before the bare name, because a name
        // followed by `(` is a call and never a field.
        call,
        // `record`, the start of `record["unit price"]`. It builds a field
        // at once, so a field of the record can be named under a quantifier
        // that binds the same name.
        map(word("record"), |_| Expr::Field(Vec::new())),
        // `amount`. A bare name is an element until `resolve` knows the
        // scope.
        map(name, |name| {
            let path = Vec::new();
            Expr::Element { name, path }
        }),
        // `100`, `"paid"` and `None`.
        literal,
        // `(a or b)` and `(1, 2)`.
        parenthesized,
        // `[1, 2]`.
        bracket_list,
    )(i, p)
}

/// `literal := "None" | "True" | "False" | number | text`
///
/// Reads `None`, `True`, `100`, `0.5` and `"paid"`.
fn literal(i: &Input<'_>, p: usize) -> Reply<Expr> {
    // `int` gives the magnitude of an integer token, and the alternative for
    // `100` below checks it against `i64`.
    let int = token("a literal", |t| match t.kind {
        Tok::Int(v) => Some(v),
        _ => None,
    });
    let float = token("a literal", |t| match t.kind {
        Tok::Float(v) => Some(Expr::Float(v)),
        _ => None,
    });
    choice!(
        // `None`, `True` and `False`.
        map(word("None"), |_| Expr::None),
        map(word("True"), |_| Expr::Bool(true)),
        map(word("False"), |_| Expr::Bool(false)),
        // `100`. A magnitude above the largest i64 is too large without a
        // minus before it: `9223372036854775808 > x` is refused at bytes 0
        // to 19.
        try_map(int, |_, v, at| {
            let reason = Reason::IntTooLarge;
            i64::try_from(v)
                .map(Expr::Int)
                .map_err(|_| SyntaxError { at, reason })
        }),
        // `0.5`.
        float,
        // `"paid"`.
        text_literal,
    )(i, p)
}

/// `text`
///
/// Reads `"paid"`. Python joins two texts that stand side by side, and a
/// rule refuses them.
fn text_literal(i: &Input<'_>, p: usize) -> Reply<Expr> {
    // `text` builds a new parser of one text token on each call. The rule
    // needs two, because a combinator takes its parser by value.
    let text = || {
        token("a literal", |t| match &t.kind {
            Tok::Text(s) => Some(s.clone()),
            _ => None,
        })
    };
    // `second` reads a second text right after the first, with its bytes:
    // `"def"`, bytes 6 to 11 of `"abc" "def" == name`. It is hidden, or
    // `"a" b` would get "expected a literal, an operator, ...".
    let second = opt(hidden(spanned(text())));
    // `value` is the first text. The last argument, the bytes of both
    // texts, is not needed.
    try_map(then(text(), second), |_, (value, second), _| match second {
        // `"paid"` alone.
        None => Ok(Expr::Text(value)),
        // `"abc" "def"`, refused at the second text.
        Some((_, at)) => {
            let reason = Reason::AdjacentTexts;
            Err(SyntaxError { at, reason })
        }
    })(i, p)
}

/// `items := (expr ("," expr)* ","?)?`
///
/// Reads the `a, b` of `(a, b)` and the `a,` of `(a,)`, and says whether a
/// comma ended the items.
fn items(i: &Input<'_>, p: usize) -> Reply<(Vec<Expr>, bool)> {
    // The first item, then each `, item`, then a comma at the end. In `(a,)`
    // the `,` has no item after it, so `many` stops before it and
    // `opt(punct(","))` reads it.
    let some = then(then(expr, many(right(punct(","), expr))), opt(punct(",")));
    map(opt(some), |items| match items {
        // `()`.
        None => (Vec::new(), false),
        // `a, b` and `a,`. `first` is `a`, `rest` holds the items after it,
        // and `comma` holds the place of a comma at the end.
        Some(((first, rest), comma)) => {
            let all = std::iter::once(first).chain(rest).collect();
            (all, comma.is_some())
        }
    })(i, p)
}

/// `"(" items ")"`
///
/// Reads `(a or b)`, which is `a or b`, and the lists `(a, b)`, `(a,)` and
/// `()`.
fn parenthesized(i: &Input<'_>, p: usize) -> Reply<Expr> {
    // `inside` reads the items between `(` and `)`, one level of nesting
    // deeper. A `(` without its `)`, as in `(a > 1 and b < 2`, is refused at
    // the `(`.
    let inside = delimited(punct("("), items, punct(")"));
    // `pop` takes the last item off, so the arms can tell one item from
    // several.
    map(inside, |(mut items, comma)| match items.pop() {
        // `(a)`: one item and no comma is the item itself.
        Some(only) if items.is_empty() && !comma => only,
        // `(a, b)` and `(a,)`: the last item goes back, and the items form
        // a list.
        Some(last) => {
            items.push(last);
            Expr::List(items)
        }
        // `()`: the empty list.
        None => Expr::List(items),
    })(i, p)
}

/// `"[" items "]"`
///
/// Reads the list `[1, 2]`.
fn bracket_list(i: &Input<'_>, p: usize) -> Reply<Expr> {
    let inside = delimited(punct("["), items, punct("]"));
    // A comma at the end changes nothing, because `[a]` is a list too.
    map(inside, |(items, _)| Expr::List(items))(i, p)
}

/// `"(" items ")"`
///
/// Reads the `(name)` of `len(name)` and the `("a")` of `x.startswith("a")`.
fn args(i: &Input<'_>, p: usize) -> Reply<Vec<Expr>> {
    // The `(` is hidden from expected sets, because after any name it is
    // one more thing that could follow.
    let inside = delimited(hidden(punct("(")), items, punct(")"));
    // A comma at the end is dropped: `len(name,)` is `len(name)`. The
    // caller counts the arguments.
    map(inside, |(items, _)| items)(i, p)
}

// Function calls.

/// How the arguments of a function are read.
#[derive(Clone, Copy)]
pub(super) enum Arguments {
    // One expression, as in `len(name)`.
    One(Unary),
    // `test for name in list`, as in `any(i > 1 for i in items)`.
    Quantifier(Quantifier),
    // A value and its types, as in `isinstance(price, (int, float))`.
    IsInstance,
    // A pattern and a text, as in `re.search(r"^a", name)`.
    Pattern(Pattern),
}

// Every function of the closed list of ADR-0041: its name, and how its
// arguments are read.
pub(super) const FUNCTIONS: [(&str, Arguments); 8] = [
    ("len", Arguments::One(Expr::Len)),
    (
        "any",
        Arguments::Quantifier(|name, list, test| Expr::Any { name, list, test }),
    ),
    (
        "all",
        Arguments::Quantifier(|name, list, test| Expr::All { name, list, test }),
    ),
    ("isinstance", Arguments::IsInstance),
    (
        "re.search",
        Arguments::Pattern(|pattern, text| Expr::ReSearch { pattern, text }),
    ),
    (
        "re.match",
        Arguments::Pattern(|pattern, text| Expr::ReMatch { pattern, text }),
    ),
    (
        "re.fullmatch",
        Arguments::Pattern(|pattern, text| Expr::ReFullmatch { pattern, text }),
    ),
    ("math.isnan", Arguments::One(Expr::IsNan)),
];

/// `call := (name | ("re" | "math") "." name) "(" arguments ")"`
///
/// Reads `len(name)`, `isinstance(price, int)` and `re.search(r"a", x)`.
/// The name picks how the arguments are read.
fn call(i: &Input<'_>, p: usize) -> Reply<Expr> {
    // 1. read the name: a plain name, or `re` or `math`, a dot and a name
    // 2. see the `(`
    // 3. look the name up
    // 4. read the arguments in the way the function asks for

    // 1. `function_name` gives the name and whether it has a module: `len`
    //    and false for `len(name)`, `re.search` and true for
    //    `re.search(r"a", x)`. `q` is the token after the name.
    let Some(((function, in_module), q)) = function_name(i, p)? else {
        // 1. No name stands here, as at the `100` of `100 > x`.
        return Ok(None);
    };
    // 1. `at` holds the bytes of the name: `lenn` is 0 to 4 of
    //    `lenn(name) > 3`.
    let at = i.span(p, q);

    // 2. A plain name without `(` is no call. The rule does not match, and
    //    `atom` reads a field, so `len == 1` is a rule about a field `len`.
    //    After `re.search` the `(` must follow, so it is not hidden there.
    let open = punct("(");
    let seen = if in_module {
        open(i, q)?
    } else {
        // 2. Hidden, or `amount b` would get "expected `(`, an operator, ...".
        hidden(open)(i, q)?
    };
    // 2. Without the `(` the rule does not match. The note of `open` gives
    //    `re.search == 1` the error "expected `(` after `search`".
    if seen.is_none() {
        return Ok(None);
    }

    // 3. An unknown name is refused over the bytes of the name, with the
    //    nearest known one, as `lenn` with `len`.
    let Some(&(name, arguments)) = FUNCTIONS.iter().find(|(n, _)| *n == function) else {
        let nearest = nearest(&function, &names(&FUNCTIONS));
        let name = function;
        let reason = Reason::UnknownFunction { name, nearest };
        let error = SyntaxError { at, reason };
        return Err(Refusal { pos: q, error });
    };

    // 4. This is the only rule that reads a name and its `(`, so no other
    //    alternative reads the same bracket again. Each arm builds its
    //    parser of the arguments and runs it from the `(` at `q`.
    match arguments {
        Arguments::One(make) => one_argument(name, make)(i, q),
        Arguments::Quantifier(make) => quantifier_arguments(name, make)(i, q),
        Arguments::IsInstance => isinstance_arguments(i, q),
        Arguments::Pattern(make) => pattern_arguments(name, make)(i, q),
    }
}

/// `name | ("re" | "math") "." name`
///
/// Reads the `len` of `len(name)` and the `re.search` of
/// `re.search(r"a", x)`, and says whether the name has a module.
fn function_name(i: &Input<'_>, p: usize) -> Reply<(String, bool)> {
    // `module` reads `re` or `math`, as the module of `re.search`. The note
    // has no name, because `hidden` below keeps it out of every error.
    let module = token("", |t| match &t.kind {
        Tok::Name(n) => MODULES.iter().find(|m| *m == n).copied(),
        _ => None,
    });
    let Some((module, q)) = hidden(module)(i, p)? else {
        // `len`: a plain name. `name` refuses a word of the language, so
        // `in(x)` is no call.
        return map(name, |name| (name, false))(i, p);
    };
    // `re.search`: the module, a dot and the function. Any name token is
    // read after the dot, so `re.if(x)` is the unknown function `re.if`.
    // `re` or `math` with no function after it names nothing.
    let Some((function, r)) = right(punct("."), any_name)(i, q)? else {
        // `re == 1` is refused at `re`, bytes 0 to 2. In `re.` the note "a
        // name" after the dot lies further and wins.
        let (at, reason) = (i.tok(p).at.clone(), Reason::ModuleAlone { word: module });
        let error = SyntaxError { at, reason };
        return Err(Refusal { pos: q, error });
    };
    // The name is joined as `FUNCTIONS` lists it, `re.search`, and `r` is
    // the token after it.
    Ok(Some(((format!("{module}.{function}"), true), r)))
}

/// `"(" expr ")"`
///
/// Reads the `(name)` of `len(name)`. Any other number of arguments is
/// refused.
fn one_argument(name: &'static str, make: Unary) -> impl Parser<Expr> {
    // `at` is the bytes of the arguments with their brackets, `(a, b)` at 3
    // to 9 in `len(a, b) > 1`.
    try_map(args, move |_, mut args, at| {
        // `pop` takes the last argument, and `given` counts all of them.
        let given = args.len();
        match args.pop() {
            // `len(name)`.
            Some(argument) if given == 1 => Ok(make(Box::new(argument))),
            // `len()` and `len(a, b)`.
            _ => {
                let takes = 1;
                let reason = Reason::WrongArgumentCount { name, takes, given };
                Err(SyntaxError { at, reason })
            }
        }
    })
}

/// `"(" expr "for" name "in" or_expr ")"`
///
/// Reads the `(i.price > 100 for i in items)` of
/// `any(i.price > 100 for i in items)`.
fn quantifier_arguments(function: &'static str, make: Quantifier) -> impl Parser<Expr> {
    // `any(i > 1)` without `for` is the mistake to name. `any(i.price >
    // 100)` is refused at the `)`, bytes 17 to 18.
    let needs_for = require(Reason::QuantifierNeedsFor { function }, word("for"));
    // `binding` reads `for i in`. An error at `in` says after which name it
    // stands, as "expected `in` after `i`" for `for i items`. `name` refuses
    // a word, so `for in items` gets "expected a name after `for`".
    let binding = right(needs_for, then_after(name, word("in")));
    // `body` reads the test, the binding, then the list. The list is read at
    // the level of `or`, as Python reads it.
    let body = then(expr, then(binding, or_expr));
    let inside = delimited(punct("("), body, punct(")"));
    // `test` is `i.price > 100`, `name` is `i`, `_` is the place of `in`
    // and `list` is `items`.
    map(inside, move |(test, ((name, _), list))| {
        make(name, Box::new(list), Box::new(test))
    })
}

/// `"(" expr "," types ","? ")"`
///
/// Reads the `(price, (int, float))` of `isinstance(price, (int, float))`.
fn isinstance_arguments(i: &Input<'_>, p: usize) -> Reply<Expr> {
    // `types` reads `, (int, float)` after the value, and a comma at the
    // end. The `types` inside the initializer is the rule `types`, because
    // the new binding starts after this statement.
    let types = opt(right(punct(","), left(types, opt(punct(",")))));
    let inside = delimited(punct("("), then(expr, types), punct(")"));
    // `at` is the bytes of the arguments with their brackets.
    try_map(inside, |_, (value, types), at| {
        // `isinstance(x)`: the types are missing. The error covers `(x)`,
        // bytes 10 to 13.
        let Some(types) = types else {
            let (name, takes, given) = ("isinstance", 2, 1);
            let reason = Reason::WrongArgumentCount { name, takes, given };
            return Err(SyntaxError { at, reason });
        };
        let value = Box::new(value);
        Ok(Expr::IsInstance { value, types })
    })(i, p)
}

// The types that `isinstance` tests for (ADR-0041).
pub(super) const TYPES: [(&str, TypeName); 5] = [
    ("int", TypeName::Int),
    ("float", TypeName::Float),
    ("str", TypeName::Str),
    ("bool", TypeName::Bool),
    ("list", TypeName::List),
];

/// `type := "int" | "float" | "str" | "bool" | "list"`
///
/// Reads the `int` of `isinstance(price, int)`. Any other name is refused
/// with the nearest known one.
fn type_name(i: &Input<'_>, p: usize) -> Reply<TypeName> {
    // Any name token is read, so a word such as `None` is an unknown type
    // too. `isinstance(price, number)` is refused at `number`, bytes 18 to
    // 24.
    try_map(any_name, |_, name, at| {
        let known = TYPES.iter().find(|(n, _)| *n == name);
        known.map(|(_, t)| *t).ok_or_else(|| {
            let nearest = nearest(&name, &names(&TYPES));
            let reason = Reason::UnknownType { name, nearest };
            SyntaxError { at, reason }
        })
    })(i, p)
}

/// `types := type | "(" type ("," type)* ","? ")"`
///
/// Reads `int` and `(int, float)`.
fn types(i: &Input<'_>, p: usize) -> Reply<Vec<TypeName>> {
    // `several` reads `int, float`: the first type, then each `, type`.
    let several = then(type_name, many(right(punct(","), type_name)));
    // `inside` reads `(int, float)` and `(int,)`, and drops a comma at the
    // end.
    let inside = delimited(punct("("), left(several, opt(punct(","))), punct(")"));
    let one_or_several = or(
        // `int`.
        map(type_name, |t| vec![t]),
        // `(int, float)`.
        map(inside, |(first, rest)| {
            std::iter::once(first).chain(rest).collect()
        }),
    );
    // An error says "a type" and does not list a name and a `(`.
    label("a type", one_or_several)(i, p)
}

/// `"(" text "," expr ","? ")"`
///
/// Reads the `(r"^a", name)` of `re.search(r"^a", name)`.
fn pattern_arguments(function: &'static str, make: Pattern) -> impl Parser<Expr> {
    let text = token("a pattern", |t| match &t.kind {
        Tok::Text(s) => Some(s.clone()),
        _ => None,
    });
    // The pattern is a text token and no expression, so it can be checked
    // when the rule is created (ADR-0041). `re.search(pattern, x)` is
    // refused at `pattern`, bytes 10 to 17.
    let pattern = require(Reason::PatternNotLiteral { function }, text);
    // `body` reads the pattern, a comma, the text to search, and a comma at
    // the end.
    let body = then(pattern, right(punct(","), left(expr, opt(punct(",")))));
    let inside = delimited(punct("("), body, punct(")"));
    map(inside, move |(pattern, text)| make(pattern, Box::new(text)))
}

/// Turns every element that no enclosing `any` or `all` binds into a field
/// of the record.
fn resolve(e: &mut Expr, scope: &mut Vec<String>) {
    match e {
        // A name that no quantifier binds is a field: `amount`, or `i.price`
        // outside any `any(... for i in ...)`.
        Expr::Element { name, path } if !scope.contains(name) => {
            // The name becomes the first key, before the steps after it.
            // `mem::take` moves it out of the element that the field
            // replaces.
            let mut full = vec![Segment::Key(mem::take(name))];
            full.append(path);
            *e = Expr::Field(full);
        }
        // The list is resolved outside the new name and the test inside it,
        // as Python scopes a generator.
        Expr::Any { name, list, test } | Expr::All { name, list, test } => {
            resolve(list, scope);
            // The name is bound while the test is resolved. The scope is a
            // stack, so an inner `any` may bind the same name again.
            scope.push(name.clone());
            resolve(test, scope);
            scope.pop();
        }
        // Every other node resolves its children. A bound element, as `i` in
        // `any(i > 1 for i in items)`, stays an element.
        _ => {
            for child in e.children_mut() {
                resolve(child, scope);
            }
        }
    }
}
