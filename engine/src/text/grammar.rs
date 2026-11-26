//! The grammar of the rule language, written with the kit, which is the set
//! of parser combinators in `kit.rs`. That file says what a note, a refusal,
//! `label` and `hidden` are. This file holds `parse`, the token tests and the
//! rules of the grammar. `parse` tries the reasons `KeywordCase` and
//! `NotInLanguage` before it returns an `Expected` error. ADR-0041 lists the
//! kinds of expression that the grammar reads.

use std::mem;
use std::ops::Range;

use super::error::{Found, Reason, SyntaxError};
use super::kit::{
    Input, Parser, Refusal, Reply, chain_left, choice, delimited, explain, hidden, label, left,
    many, map, nested, opt, or, require, right, spanned, then, then_after, token, try_map,
};
use super::tokens::{Tok, lex};
use crate::expr::{Expr, Segment, TypeName};

// The twelve reserved words of ADR-0041 and the module name `re`. `name` does
// not match a name token that is one of them, so a field of that name is
// written `record["in"]`. The printer writes such a key in brackets for the
// same reason.
pub(super) const WORDS: [&str; 13] = [
    "and", "or", "not", "in", "is", "if", "else", "for", "None", "True", "False", "record", "re",
];

// ADR-0041 reserves `re` as the name of the module of the pattern functions.
// Its reason is that a call would otherwise need four tokens to tell it from a
// field: `re.search(` and the field `re.search` differ at the `(`.
// `function_name` reads `re` only as the start of a call such as `re.search`,
// and a bare `re` is refused with `ModuleAlone`. The constant is a list, so a
// later module joins it as one more entry, and `ModuleAlone` names the module
// it refuses.
const MODULES: [&str; 1] = ["re"];

// Python's keywords (`keyword.kwlist`) less those that are words of the
// language. ADR-0041 makes them names, and a rule may use one as a key,
// because the language does not aim to ban Python's other keywords. They are
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
    // Parsing splits the text into tokens, runs the grammar, and then either
    // resolves the names of the tree or builds the error.
    //
    // 1. We split the text into tokens. `lex` does not fail. A text it
    //    cannot split ends in a `Bad` token, and the grammar reports it only
    //    when it gets there.
    // 2. We run the grammar from the first token. It gives a tree, no match,
    //    or a refusal such as `UnknownFunction` for `lenn(name) > 3`.
    // 3. On a tree, we resolve the names. The grammar holds every bare name
    //    as an `Expr::Element`, the node for the element that a quantifier
    //    names, because in `any(test for name in list)` the name comes after
    //    the test that uses it.
    // 4. Otherwise we build the error from the refusal and from the log of
    //    the input, in which each token test that did not match noted what
    //    it wanted. Before we return an `Expected` error, we try the reasons
    //    `KeywordCase` and `NotInLanguage`, and neither changes which texts
    //    are valid.

    // 1. split into tokens
    let input = Input::new(text, lex(text));

    // 2. run the grammar
    let refusal = match rule(&input, 0) {
        Ok(Some((mut tree, _))) => {
            // 3. resolve the names
            resolve(&mut tree, &mut Vec::new());
            return Ok(tree);
        }
        // Without a match there is no refusal, and the error comes from the
        // log alone, as for `a > 1 b`.
        Ok(None) => None,
        Err(refusal) => Some(refusal),
    };

    // 4. build the error
    let error = explain(&input, refusal);
    // A refusal or a tokenizer's reason already says what is wrong, as for
    // `0 < amount < 10`.
    if !matches!(error.reason, Reason::Expected { .. }) {
        return Err(error);
    }
    // `KeywordCase` for the word at the failure is tried before
    // `NotInLanguage` for the word before it, because the author has to edit
    // the word at the failure. `AND` in `a > 1 AND b < 2` gets the first and
    // `lambda` in `lambda x: x > 1` the second, and `lambda AND b` the first.
    let targeted = miscased_word(&input).or_else(|| python_only_word(&input));
    // When neither applies, the `Expected` error gets what the grammar knows
    // of the name at its place, as "did you mean `and`?" for `adn`.
    Err(targeted.unwrap_or_else(|| with_words(&input, error)))
}

/// Returns a `KeywordCase` error over the name token at the furthest place,
/// when that name differs only in case from a word of the language that some
/// test wanted there, and `None` otherwise.
fn miscased_word(i: &Input<'_>) -> Option<SyntaxError> {
    // `wanted` also holds the notes that `label` and `hidden` keep out of
    // the error. In `x NOT in y` it holds `not`, which the label "an
    // operator" of `compare_tail` took out of the error.
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
    let at = token.at.clone();
    // The word must be one that a test wanted here, and `word` notes a word
    // in backticks. In `a > 1 Record b` no test wanted `record`, so `parse`
    // goes on to `python_only_word` and then to the `Expected` error.
    wanted
        .contains(&format!("`{keyword}`"))
        .then_some(SyntaxError { at, reason })
}

/// Returns a `NotInLanguage` error over the token before the furthest place,
/// when that token is a Python keyword outside the language, and `None`
/// otherwise.
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
    let at = token.at.clone();
    PYTHON_ONLY
        .contains(&word.as_str())
        .then_some(SyntaxError { at, reason })
}

/// For an `Expected` error whose place holds a name token, returns the error
/// with `found` set to `Found::Word` when the name is a word of the language,
/// and otherwise with `nearest` set to the nearest word that some test wanted
/// there. Any other error comes back unchanged.
fn with_words(i: &Input<'_>, error: SyntaxError) -> SyntaxError {
    let (pos, wanted) = i.furthest();
    // Only a name token at the place gets a word or a guess, so `a > )`
    // returns its error as it is. `parse` passes only an `Expected` error,
    // which `explain` built at the token at `pos`, so the second half of the
    // pattern always matches.
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
    // Only a word that some test wanted at this place is offered. After the
    // `x` of `x iss None` the label "an operator" took `is` out of the error,
    // and `is` is still offered.
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

/// Any name token, also a word of the language. The rules for a name after
/// a dot and for the name of a type read with it, so that a word of the
/// language there gets an error of its own.
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
    // A text with no token, empty or only spaces, is refused with `Empty`
    // over the whole text. The tokenizer skips spaces, so the guard tests for
    // `End` at the first token.
    if matches!(i.tok(p).kind, Tok::End) {
        let (at, reason) = (0..i.text.len(), Reason::Empty);
        let error = SyntaxError { at, reason };
        return Err(Refusal { pos: p, error });
    }
    // An error at the end says after which expression it stands, as in
    // "after `a > 1`" for `a > 1 b`.
    map(then_after(expr, end_of_rule), |(e, ())| e)(i, p)
}

/// `end := <the end of the text>`
///
/// Reads the end of `amount > 100`. In `a > 1) and b` it refuses the `)`.
fn end_of_rule(i: &Input<'_>, p: usize) -> Reply<()> {
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
    // consumes its own. The refusal covers the `)` of `a > 1) and b`.
    let stray = try_map(closer, |_, close, at| {
        let reason = Reason::ClosesNothing { close };
        Err(SyntaxError { at, reason })
    });
    // The stray bracket is read only to refuse it. `hidden` keeps its test
    // out of the list of what could stand after a complete rule, so
    // `a > 1 b < 2` gets "expected `and`, `or`, `if` or the end of the rule"
    // with no bracket.
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
    // A missing `else` gets a refusal of its own, `ConditionalNeedsElse`.
    // `price if price > 0` is refused at the end of the text.
    let needs_else = require(Reason::ConditionalNeedsElse, word("else"));
    // The test is read at the level of `or`, so a conditional in it needs
    // parentheses. The branch after `else` goes through `nested`, so that the
    // cap on nesting holds where this rule calls itself.
    let tail = then(right(word("if"), or_expr), nested(needs_else, conditional));
    // `first` is the value before `if`, `price` in the example.
    map(then(or_expr, opt(tail)), |(first, tail)| match tail {
        // `a or b`, with no `if` after it.
        None => first,
        // `tail` gave the test and the branch after `else`, and the node
        // reads `then if test else otherwise`.
        Some((test, otherwise)) => Expr::If {
            test: Box::new(test),
            then: Box::new(first),
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
    // `kind` is the kind of node that `make` builds, `And` or `Or`, which the
    // function learns by building an empty node. It does not compare `make`
    // with `Expr::And`, because Rust does not promise that two pointers to
    // one function compare equal.
    let kind = mem::discriminant(&make(Vec::new()));
    // `operands` gives the operands that one side brings into the list.
    let operands = |e: Expr| {
        let same = mem::discriminant(&e) == kind;
        match e {
            // A node of the same kind brings its own operands: the left side
            // `a and b` of `(a and b) and c` brings `a` and `b`.
            Expr::And(inner) | Expr::Or(inner) if same => inner,
            // Anything else is one operand. That includes a node of the other
            // kind, as `(c or d)` in `a and (c or d)`.
            e => vec![e],
        }
    };
    let mut all = operands(l);
    all.extend(operands(r));
    make(all)
}

/// `not_expr := "not" not_expr | comparison`
///
/// Reads `not active` and `not not x`. A `not` that follows a value starts
/// the operator `not in`, which `comparison` reads, so this rule reads only a
/// `not` that stands where a value starts.
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

// The six comparisons of the language (ADR-0041), each with the builder of
// its node. `symbol` compares whole operator tokens, so the order of the
// entries does not matter.
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
    // `additive` reads the operand, so in `0 < amount < 10` it stops before
    // the second `<`.
    let operator_and_operand = then(spanned(compare_op), spanned(additive));
    // The value is the operator and the right operand, each with its bytes.
    // `build_comparison` uses the bytes for its errors.
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
/// after it with its place. The outer `None` means that no second operator
/// followed the comparison. The inner `None` means that a second operator
/// stood and no operand followed it, as in `a < b <`.
type Chained = Option<(Placed<()>, Option<Placed<Expr>>)>;

/// `comparison := additive compare_tail?`
///
/// Reads `amount > 100`, `x not in (1, 2)` and `discount is not None`, or
/// the operand alone. Comparisons do not chain, so `0 < amount < 10` is
/// refused.
fn comparison(i: &Input<'_>, p: usize) -> Reply<Expr> {
    // `again` reads a second operator, as the second `<` of
    // `0 < amount < 10` or the second `is` of `x is None is None`. It is
    // hidden, or the error for `a > 1 b` would list "an operator", `in`,
    // `not` and `is`.
    let again = hidden(or(map(compare_op, |_| ()), map(word("is"), |_| ())));
    // The second operator is read so that `build_comparison` refuses every
    // chain with `ChainedComparison`, because the language has no chained
    // comparisons (ADR-0041). The third operand is optional, so that
    // `a < b <` is still refused as a chain, at its second `<`.
    let chained = opt(then(spanned(again), opt(spanned(additive))));
    // `tail` reads what follows the first operand: `< amount` and the chain
    // after it. In `amount and x` nothing follows.
    let tail = opt(then(compare_tail, chained));
    try_map(then(spanned(additive), tail), build_comparison)(i, p)
}

/// The node of a comparison from its parts, or the refusal of a chain of
/// comparisons.
fn build_comparison(
    i: &Input<'_>,
    ((left, left_at), tail): (Placed<Expr>, Option<(Tail, Chained)>),
    // The bytes of the whole comparison, which `try_map` passes and no error
    // here needs.
    _: Range<usize>,
) -> Result<Expr, SyntaxError> {
    // The function builds the node of a comparison from the first operand and
    // the tail that `comparison` read, or it refuses the text. It tests these
    // cases in order: no operator followed, a second operator followed, an
    // `is None` or `is not None` tail, and a binary tail, which becomes the
    // node. The place of the second case matters, and the note there says
    // why.
    let text = |at: &Range<usize>| i.slice(at);
    // No operator followed: `amount` in `amount and x` is the operand alone.
    let Some((tail, chained)) = tail else {
        return Ok(left);
    };
    // A second operator stood at `again_at`, and `third` is the optional
    // third operand with its bytes. This case comes before the `is None`
    // arms, which return a complete node, or `x is None is None` would parse
    // as `IsNone` and drop its second `is`.
    if let Some((((), again_at), third)) = chained {
        // The rewrite needs two comparisons of two operands each. It repeats
        // the operands as written and does not judge them. So the rewrite of
        // `None < x < 10` parses, and validation then refuses its `None`
        // (ADR-0042).
        let write = match (&tail, third) {
            (Tail::Binary(_, op_at, _, right_at), Some((_, third_at))) => {
                // `first` is `0 < amount`, written from the three parts.
                let first = format!("{} {} {}", text(&left_at), text(op_at), text(right_at));
                // The middle operand stands in both comparisons, so `second`
                // is `amount < 10`.
                let second = format!("{} {} {}", text(right_at), text(&again_at), text(&third_at));
                Some(format!("{first} and {second}"))
            }
            // `a < b <` and `x is None is None` get no rewrite.
            _ => None,
        };
        let reason = Reason::ChainedComparison { write };
        return Err(SyntaxError {
            at: again_at,
            reason,
        });
    }
    match tail {
        Tail::IsNone(false) => Ok(Expr::IsNone(Box::new(left))),
        Tail::IsNone(true) => Ok(Expr::IsNotNone(Box::new(left))),
        // The literal `None` is an operand like any other here. Validation
        // refuses it beside an ordering and after `in` and `not in`
        // (ADR-0042).
        Tail::Binary(make, _, right, _) => Ok(make(Box::new(left), Box::new(right))),
    }
}

// The arithmetic operators of each level, each with the builder of its node.
// The two levels follow Python's precedence (ADR-0041). The set holds the
// operators that rules need so far, and another operator can be added later.
const ADDITIVE: [(&str, Binary); 2] = [("+", Expr::Add), ("-", Expr::Sub)];
const TERM: [(&str, Binary); 3] = [("*", Expr::Mul), ("/", Expr::Div), ("%", Expr::Mod)];

/// `additive := term (("+" | "-") term)*`
///
/// Reads `a - b + 1`, grouped to the left as `(a - b) + 1`.
fn additive(i: &Input<'_>, p: usize) -> Reply<Expr> {
    // The operator is hidden, because an arithmetic operator may follow
    // almost any value and would bury the keywords in every "expected" error.
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
/// Reads a postfix form, or a leading minus before one. On a number token the
/// minus gives a negative literal, as in `-5`, and on anything else a `Neg`,
/// as in `-a` and `-(5)`. Refuses an integer below the smallest i64.
fn unary(i: &Input<'_>, p: usize) -> Reply<Expr> {
    // `int` gives the magnitude of an integer token, a u64. The tokenizer
    // reads a number without its sign.
    let int = token("a number", |t| match t.kind {
        Tok::Int(v) => Some(v),
        _ => None,
    });
    let float = token("a number", |t| match t.kind {
        Tok::Float(v) => Some(v),
        _ => None,
    });
    // The two number alternatives come before `- unary`, because the choice
    // takes the first that matches, and `- unary` would read `-5` as `Neg` of
    // `5`.
    let minus_or_postfix = choice!(
        // `-5`. The minus folds only into a number token, so `-(5)` stays a
        // `Neg` and a printed tree parses back to itself. The number is
        // hidden, or `- )` would get "expected a number or a value". No step
        // is read after such a literal, so `-5[0]` is refused at the `[`.
        try_map(right(punct("-"), hidden(int)), |_, v, at| {
            let reason = Reason::IntTooLarge;
            // Subtracting the magnitude from 0 fails above 2^63, so the
            // smallest integer, with the magnitude 2^63, can still be
            // written. The error covers the minus and the number.
            let negative = 0_i64.checked_sub_unsigned(v);
            negative.map(Expr::Int).ok_or(SyntaxError { at, reason })
        }),
        // `-0.5`.
        map(right(punct("-"), hidden(float)), |v| Expr::Float(-v)),
        // `-a` and `- -5`. Each minus that this alternative reads opens a
        // level of nesting. In `- -5` only the first does, because the second
        // folds into the literal `-5`.
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
    // The steps are hidden, because a `.` or a `[` may follow almost any
    // value and would show up in every "expected" error. A step that fails
    // after its first token still reports, as `customer..country` with
    // "expected a name".
    let steps = many(spanned(hidden(or(dot_step, bracket_step))));
    try_map(then(spanned(atom), steps), apply_postfixes)(i, p)
}

/// `"." name ("(" items ")")?`
///
/// Reads the `.price` of `item.price` and the `.upper()` of `name.upper()`.
fn dot_step(i: &Input<'_>, p: usize) -> Reply<Post> {
    // Any name token is read here, also a word of the language, so that
    // `flows.in` gets a reason of its own in `apply_postfixes`.
    let step = then(spanned(any_name), opt(spanned(args)));
    // The value is the name with its bytes, then the arguments with theirs
    // when a `(` follows.
    map(right(punct("."), step), |((name, at), call)| match call {
        // `.upper()`: a method call. A word of the language before a `(` is
        // read as a method name too, so `flows.in()` gets `UnknownMethod`.
        Some((args, args_at)) => Post::Call(name, at, args, args_at),
        // `.in`: a word of the language after a dot is never a key
        // (ADR-0041).
        None if WORDS.contains(&name.as_str()) => Post::Word(name, at),
        // `.price`: a key.
        None => Post::Step(Segment::Key(name)),
    })(i, p)
}

/// `"[" (index | text) "]"`
///
/// Reads the `[0]` of `items[0]` and the `["in"]` of `flows["in"]`. A
/// negative index such as `[-1]` is refused with a reason of its own.
fn bracket_step(i: &Input<'_>, p: usize) -> Reply<Post> {
    // `step` reads the `0` of `items[0]` or the `"in"` of `flows["in"]`.
    let step = token("an index or a key", |t| match &t.kind {
        // A path index is a u32. An index above `u32::MAX` does not match,
        // so `items[99999999999]` gets "expected an index or a key".
        Tok::Int(v) => u32::try_from(*v).ok().map(Segment::Index),
        Tok::Text(s) => Some(Segment::Key(s.clone())),
        _ => None,
    });
    // `number` reads any integer token, as the `1` of `[-1]`.
    let number = token("a number", |t| matches!(t.kind, Tok::Int(_)).then_some(()));
    // `[-1]`. Python's index from the end is refused by name (ADR-0041). The
    // refusal covers the `-1` of `items[-1].price > 5`.
    let negative = try_map(then(punct("-"), number), |_, _, at| {
        let reason = Reason::NegativeIndex;
        // The closure always refuses. The turbofish names `Segment` as the
        // value it would give, so both alternatives below give a `Segment`.
        Err::<Segment, _>(SyntaxError { at, reason })
    });
    // `inside` reads the index or the key, then the `]`. The negative index
    // is hidden, or `items[x]` would get "expected an index or a key or
    // `-`". A `-` with no number after it still reports, as `items[-x]` with
    // "expected a number after `-`".
    let inside = left(or(step, hidden(negative)), punct("]"));
    map(right(punct("["), inside), Post::Step)(i, p)
}

/// Applies the postfix steps to the atom, left to right, and returns the
/// expression with every step applied.
///
/// It refuses `record` with no key, or with an index or a method
/// (`BareRecord`), a key or an index on a value that is no path
/// (`KeyOnValue`), a word of the language after a dot (`WordAfterDot`), and
/// a method call that `method` refuses.
fn apply_postfixes(
    i: &Input<'_>,
    // The atom comes with its bytes, and `posts` holds each step with its
    // bytes.
    ((atom, atom_at), posts): (Placed<Expr>, Vec<Placed<Post>>),
    // The bytes of the whole form, which no error here needs.
    _: Range<usize>,
) -> Result<Expr, SyntaxError> {
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
            // key. This arm comes first, because a bare `record` is an
            // `Expr::Field` with an empty path, and the next arm would push
            // the index onto it.
            (Post::Step(Segment::Index(_)), e) if bare_record(&e) => {
                // The error covers `record[0]`, bytes 0 to 9.
                return refuse(atom_at.start..at.end, Reason::BareRecord);
            }
            // A step on a path that starts at `record`, as the first key of
            // `record["unit price"]` and each step after it. A bare name
            // such as `customer` is an `Expr::Element` here and takes the
            // next arm.
            (Post::Step(step), Expr::Field(mut path)) => {
                path.push(step);
                Expr::Field(path)
            }
            // A step on a bare name, as `.price` on `i` in `any(i.price > 100
            // for i in items)`. `resolve` runs on the finished tree and decides
            // whether `i` is the element of a quantifier or a field.
            (Post::Step(step), Expr::Element { name, mut path }) => {
                path.push(step);
                Expr::Element { name, path }
            }
            // Only a path has keys and indexes, so `"abc".x == 1` is refused
            // at `.x`.
            (Post::Step(_), _) => return refuse(at, Reason::KeyOnValue),
            // `flows.in` is refused at `in`. The arm takes any expression so
            // far, so `"abc".in` is told to write `"abc"["in"]`, which the
            // parser refuses as a key on a value.
            (Post::Word(word, word_at), _) => {
                // `at` starts at the dot of this step, so `before` holds the
                // atom and the earlier steps, as `flows.x` in `flows.x.in`.
                // `trim_end` drops the space of `flows .in`.
                let before = i.slice(&(atom_at.start..at.start)).trim_end();
                // The bracket form of the same key, as `flows["in"]`.
                let write = format!("{before}[\"{word}\"]");
                return refuse(word_at, Reason::WordAfterDot { word, write });
            }
            // `record.upper()`: a method on the whole record is refused at
            // `record`.
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
    // `record` with no key is the whole record, which is no value.
    // `record == 1` is refused at `record`.
    if bare_record(&e) {
        return refuse(atom_at, Reason::BareRecord);
    }
    Ok(e)
}

/// How many arguments a method takes, with the builder of its node. `Zero`
/// builds the node from the receiver alone, and `One` from the receiver and
/// the one argument.
#[derive(Clone, Copy)]
pub(super) enum Arity {
    Zero(Unary),
    One(Binary),
}

// The methods of the closed list of ADR-0041: the name an author types, and
// the builder. `error.rs` reads the names for the sentence of an unknown
// method.
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
    // The error takes the table's name, because `WrongArgumentCount` holds
    // a `&'static str`.
    let Some(&(known, arity)) = METHODS.iter().find(|(n, _)| *n == name) else {
        // `name.startwith("a")` is refused at `startwith`, with the nearest
        // method `startswith`.
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
    // `pop` gives the last argument by value, or none when there is none.
    // `given` was counted before the pop, so the arms tell no argument, one
    // argument and more apart.
    match (arity, args.pop()) {
        // `name.upper()`.
        (Arity::Zero(make), None) => Ok(make(receiver)),
        // `name.startswith("a")`, with one argument and no more.
        (Arity::One(make), Some(argument)) if given == 1 => Ok(make(receiver, Box::new(argument))),
        // `name.upper("x")` is refused at `("x")`, and so is
        // `name.startswith()` at `()`.
        _ => {
            let reason = Reason::WrongArgumentCount {
                name: known,
                takes,
                given,
            };
            Err(SyntaxError {
                at: args_at,
                reason,
            })
        }
    }
}

/// The names of a table.
pub(super) fn names<T>(table: &[(&'static str, T)]) -> Vec<&'static str> {
    table.iter().map(|(name, _)| *name).collect()
}

/// The known name nearest to `word`, when it is near enough to be a typo:
/// one edit per four characters, and at least one. Two names that are
/// equally near give none, and so does an empty list of known names.
fn nearest<'a>(word: &str, known: &[&'a str]) -> Option<&'a str> {
    // `lenn` may be one edit off and `startwith` two. One edit per four
    // characters is a chosen value that no measurement backs. The guess has a
    // limit on purpose, so that a far name is not offered.
    let limit = (word.chars().count() / 4).max(1);
    let distances: Vec<(usize, &str)> =
        known.iter().map(|k| (edit_distance(word, k), *k)).collect();
    // An empty list of known names gives none.
    let (least, name) = *distances.iter().min_by_key(|(d, _)| *d)?;
    // Two names at the same least distance give no guess, because neither is
    // the better guess. `iz` is one edit from `is`, `in` and `if`.
    let unique = distances.iter().filter(|(d, _)| *d == least).count() == 1;
    // `abs` is two edits from `all` and `any`, over its limit of one, so
    // `abs(a - b)` gets no guess, and the sentence of `UnknownFunction` lists
    // every function.
    (least <= limit && unique).then_some(name)
}

/// The optimal string alignment distance of `a` and `b`: the fewest
/// insertions, deletions, replacements and swaps of neighbours that turn `a`
/// into `b`, where no character is edited again after a swap.
fn edit_distance(a: &str, b: &str) -> usize {
    // The function computes the optimal string alignment distance, which is
    // the restricted variant of the Damerau-Levenshtein distance, by dynamic
    // programming over a full table of prefixes. `rows[i][j]` is the number
    // of edits that turn the first `i` characters of `a` into the first `j`
    // characters of `b`, so the last cell holds the distance of the whole
    // words.

    // The words become lists of characters, so a character of several bytes
    // counts as one.
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut rows = vec![vec![0; b.len() + 1]; a.len() + 1];
    for i in 0..=a.len() {
        for j in 0..=b.len() {
            if i == 0 || j == 0 {
                rows[i][j] = i + j;
                continue;
            }
            let replaced = rows[i - 1][j - 1] + usize::from(a[i - 1] != b[j - 1]);
            let mut least = replaced.min(rows[i - 1][j] + 1).min(rows[i][j - 1] + 1);
            // When the last two characters of the two prefixes are the same
            // pair in the other order, one swap of them is also a candidate,
            // so `adn` is one edit from `and` and gets the guess `and`.
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                least = least.min(rows[i - 2][j - 2] + 1);
            }
            rows[i][j] = least;
        }
    }
    rows[a.len()][b.len()]
}

// Atoms.

/// `atom := call | "record" | name | literal | "(" items ")" | "[" items "]"`
///
/// Reads `len(name)`, `record`, `amount`, `100`, `(a or b)` and `[1, 2]`.
fn atom(i: &Input<'_>, p: usize) -> Reply<Expr> {
    // The rule tries the forms of an atom in order, and the first that
    // matches wins. Only the place of `call` matters, ahead of the bare name,
    // and the note at `call` says why. The other forms start with different
    // tokens, since `name` does not match `record`, `None`, `True` and
    // `False`.
    choice!(
        // `len(name)`. The calls come before the bare name, because a name
        // followed by `(` is a call and never a field.
        call,
        // `record`, the start of `record["unit price"]`. It builds an
        // `Expr::Field` with an empty path, which stands for the record until
        // a key follows. `resolve` leaves a field alone, so `record.i` stays
        // a field of the record under `any(i > record.i for i in items)`.
        map(word("record"), |_| Expr::Field(Vec::new())),
        // `amount`. A bare name is an `Expr::Element` until `resolve` knows
        // the scope. It stays an element under a quantifier that binds the
        // name, and it becomes a field of the record otherwise.
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
    // `int` gives the magnitude of an integer token, a u64.
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
        // `100`. `unary` reads a negating minus and its number as one
        // literal before this rule runs, so a number here is not negated. Its
        // magnitude is then too large above the largest i64, as in
        // `9223372036854775808 > x`.
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
/// Reads `"paid"`. Python joins two texts that stand side by side, and the
/// rule language refuses them.
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
    // deeper. `delimited` refuses at the `(` only when the items matched and
    // the text ends where the `)` should stand, as in `(a > 1 and b < 2`.
    // Any other token there gives an ordinary "expected" error.
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
    // The `(` is hidden, so that it does not show up in the "expected" error
    // after a name that arguments may follow, as the name after a dot in
    // `x.price`.
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
    // A value and its types, as in `isinstance(price, (int, float))`. It
    // holds no builder, because `isinstance_arguments` builds the node
    // itself from types that are no expressions.
    IsInstance,
    // A pattern and a text, as in `re.search(r"^a", name)`.
    Pattern(Pattern),
}

// Every function of the closed list of ADR-0041: its name, and how its
// arguments are read.
pub(super) const FUNCTIONS: [(&str, Arguments); 7] = [
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
];

/// `call := (name | "re" "." name) "(" arguments ")"`
///
/// Reads `len(name)`, `isinstance(price, int)` and `re.search(r"a", x)`.
/// The name picks how the arguments are read. After the dot of a module any
/// name token is read, also a word of the language.
///
/// A called name outside the closed list is refused with `UnknownFunction`,
/// with the nearest known function when one is near enough. `re` without a
/// dot after it is refused with `ModuleAlone`.
fn call(i: &Input<'_>, p: usize) -> Reply<Expr> {
    // The function reads a function name, tests for the `(` after it without
    // moving past it, and only then looks the name up in `FUNCTIONS`. Then
    // it runs the parser of the arguments that the entry names.
    //
    // 1. We read the name, which may have the module `re`. A bare `re` ends
    //    the parse with its refusal.
    // 2. We test for the `(` at the token after the name, without moving
    //    past it. Without a `(` the rule gives no match. A plain name
    //    without `(` is no call, so the bare-name alternative of `atom`
    //    reads it as a field, and `len == 1` is a rule about a field `len`.
    // 3. We look the name up in `FUNCTIONS`, and refuse an unknown name with
    //    the nearest known function. The lookup comes after the `(`, so only
    //    a name that is called can be an unknown function.
    // 4. We run the parser of the arguments that the entry names, from the
    //    `(` at `q`. Step 2 did not move past the `(`, so that parser is the
    //    only one that reads the bracketed text. Among the alternatives of
    //    `atom`, `call` alone reads a name and its `(`, so no level reads its
    //    bracketed text twice, which would double the parse time per level.

    // 1. read the name
    // `function_name` gives the name and whether it has a module, `len` and
    // false for `len(name)`. `q` is the index of the token after the name.
    let Some(((function, in_module), q)) = function_name(i, p)? else {
        return Ok(None);
    };
    // `at` holds the bytes of the name, 0 to 4 of `lenn(name) > 3`, which the
    // refusal of an unknown name covers.
    let at = i.span(p, q);

    // 2. test for the `(`
    let open = punct("(");
    // After `re.search` the `(` must follow, so it is not hidden there, and
    // `re.search == 1` gets "expected `(` after `search`". `explain` takes
    // the `search` from the token before the place.
    let seen = if in_module {
        open(i, q)?
    } else {
        // The `(` is hidden here, or `amount b` would get "expected `(`, an
        // operator, ...".
        hidden(open)(i, q)?
    };
    // After a module name no other alternative of `atom` matches either,
    // because `re` is a word of the language. `open` logged the note "`(`",
    // and `explain` in `parse` builds the error from that log.
    if seen.is_none() {
        return Ok(None);
    }

    // 3. look the name up
    // The table's `name` is a `&'static str`, which the argument parsers and
    // their reasons hold, and `function` is a `String`.
    let Some(&(name, arguments)) = FUNCTIONS.iter().find(|(n, _)| *n == function) else {
        // The refusal covers the bytes of the name and gives the nearest
        // known one, as `len` for `lenn`.
        let nearest = nearest(&function, &names(&FUNCTIONS));
        let name = function;
        let reason = Reason::UnknownFunction { name, nearest };
        let error = SyntaxError { at, reason };
        // The refusal stands at `q`, the token after the name, as `try_map`
        // puts its refusal after the tokens that its parser read.
        return Err(Refusal { pos: q, error });
    };

    // 4. run the parser of the arguments
    match arguments {
        Arguments::One(make) => one_argument(name, make)(i, q),
        Arguments::Quantifier(make) => quantifier_arguments(name, make)(i, q),
        Arguments::IsInstance => isinstance_arguments(i, q),
        Arguments::Pattern(make) => pattern_arguments(name, make)(i, q),
    }
}

/// `name | "re" "." name`
///
/// Reads the `len` of `len(name)` and the `re.search` of
/// `re.search(r"a", x)`, and says whether the name has a module. After the
/// dot any name token is read, also a word of the language. `re` without a
/// dot after it is refused with `ModuleAlone`.
fn function_name(i: &Input<'_>, p: usize) -> Reply<(String, bool)> {
    // `module` reads `re`, as the module of `re.search`. Its note is empty,
    // because `hidden` below keeps it out of every error. `furthest` still
    // returns the empty note to `miscased_word` and `with_words`, which look
    // only for words in backticks, so the empty note never matches.
    let module = token("", |t| match &t.kind {
        Tok::Name(n) => MODULES.iter().find(|m| *m == n).copied(),
        _ => None,
    });
    let Some((module, q)) = hidden(module)(i, p)? else {
        // `len`: a plain name. `name` does not match a word of the language,
        // so `in(x)` is no call.
        return map(name, |name| (name, false))(i, p);
    };
    // `re.search`: the module, a dot and the function. Any name token is
    // read after the dot, so `re.if(x)` is the unknown function `re.if`.
    let Some((function, r)) = right(punct("."), any_name)(i, q)? else {
        // `re` without a dot after it is refused with `ModuleAlone`, as
        // `re == 1` is refused at `re`.
        let (at, reason) = (i.tok(p).at.clone(), Reason::ModuleAlone { word: module });
        let error = SyntaxError { at, reason };
        // The refusal stands at the token after the module. In `re.` that is
        // the dot, so the note "a name" after the dot lies further and
        // `explain` reports that note.
        return Err(Refusal { pos: q, error });
    };
    // The name is joined as `FUNCTIONS` lists it, `re.search`, and `r` is
    // the index of the token after the function.
    Ok(Some(((format!("{module}.{function}"), true), r)))
}

/// `"(" expr ","? ")"`
///
/// Reads the `(name)` of `len(name)`, and `len(name,)` reads as `len(name)`.
/// Any other number of arguments is refused.
fn one_argument(name: &'static str, make: Unary) -> impl Parser<Expr> {
    // `at` is the bytes of the arguments with their brackets, `(a, b)` in
    // `len(a, b) > 1`.
    try_map(args, move |_, mut args, at| {
        // `given` counts the arguments before `pop` takes the last one off,
        // so the count in the error includes it.
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
    // `require` turns a missing `for` into the refusal `QuantifierNeedsFor`
    // at the token where `for` should stand, as the `)` of
    // `any(i.price > 100)`.
    let needs_for = require(Reason::QuantifierNeedsFor { function }, word("for"));
    // `binding` reads `for i in`. An error at `in` says after which name it
    // stands, as "expected `in` after `i`" for `for i items`. `name` does not
    // match a word, so `for in items` gets "expected a name after `for`".
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
    // `after_value` reads `, (int, float)` after the value, and a comma at
    // the end. The types are optional, so that `isinstance(x)` gets
    // `WrongArgumentCount` below, with one argument given, where a missing
    // `,` would otherwise give an "expected" error.
    let after_value = opt(right(punct(","), left(types, opt(punct(",")))));
    let inside = delimited(punct("("), then(expr, after_value), punct(")"));
    // `at` is the bytes of the arguments with their brackets.
    try_map(inside, |_, (value, types), at| {
        // `isinstance(x)`: the types are missing, and the error covers
        // `(x)`.
        let Some(types) = types else {
            // `isinstance` takes the value and its types, and only the value
            // was given.
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
/// Reads the `int` of `isinstance(price, int)`. Any other name is refused,
/// with the nearest known type when one is near enough to be a typo.
fn type_name(i: &Input<'_>, p: usize) -> Reply<TypeName> {
    // Any name token is read, so a word such as `None` is an unknown type
    // too. `isinstance(price, number)` is refused at `number`.
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
    // The note "a pattern" reaches no error. `require` below refuses when
    // this test does not match, and at a bad token `explain` reports the
    // reason of the tokenizer.
    let text_token = token("a pattern", |t| match &t.kind {
        Tok::Text(s) => Some(s.clone()),
        _ => None,
    });
    // The pattern is a text token and no expression, so it can be checked
    // when the rule is created (ADR-0041). `re.search(pattern, x)` is
    // refused at `pattern`.
    let pattern = require(Reason::PatternNotLiteral { function }, text_token);
    // `body` reads the pattern, a comma, the text to search, and a comma at
    // the end.
    let body = then(pattern, right(punct(","), left(expr, opt(punct(",")))));
    let inside = delimited(punct("("), body, punct(")"));
    map(inside, move |(pattern, text)| make(pattern, Box::new(text)))
}

/// Turns every element that no enclosing `any` or `all` binds into a field
/// of the record. The grammar builds an `Expr::Element` for every bare name.
/// `scope` holds the names that the quantifiers around `e` already bind, so
/// it is empty for a whole tree, and the function gives it back as it found
/// it.
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
        // `resolve` resolves the list before it binds the name of the
        // quantifier, and the test after, as Python scopes a generator. In
        // `any(i > 1 for i in i.items)` the list `i.items` is a field of the
        // record.
        Expr::Any { name, list, test } | Expr::All { name, list, test } => {
            resolve(list, scope);
            // The name is bound while the test is resolved. The scope is a
            // stack, so an inner `any` may bind the same name again.
            scope.push(name.clone());
            resolve(test, scope);
            scope.pop();
        }
        // For every other node, `resolve` resolves each child. A bound
        // element, as `i` in `any(i > 1 for i in items)`, reaches this arm
        // and stays an element.
        _ => {
            for child in e.children_mut() {
                resolve(child, scope);
            }
        }
    }
}
