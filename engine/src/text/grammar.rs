//! The grammar of the rule language, written with the kit: one function per
//! level of precedence, from the lowest up. The kinds of expression are
//! ADR-0041.

use std::mem;
use std::ops::Range;

use super::error::{Reason, SyntaxError};
use super::kit::{
    Input, Parser, Refusal, Reply, chain_left, choice, delimited, explain, hidden, label, left,
    many, map, nested, opt, or, require, right, spanned, then, then_after, token, try_map,
};
use super::tokens::{Tok, lex};
use crate::expr::{Expr, Segment};

// The words of the language (ADR-0041). A name token that is one of them is
// no name.
const WORDS: [&str; 12] = [
    "and", "or", "not", "in", "is", "if", "else", "for", "None", "True", "False", "record",
];

/// A value with the bytes of the text it came from.
type Placed<T> = (T, Range<usize>);
/// The builder of a node with two operands.
type Binary = fn(Box<Expr>, Box<Expr>) -> Expr;

/// Parses `text` as a rule. Returns the tree, or the first error with its
/// place.
///
/// # Errors
///
/// The first syntax error in `text`.
pub fn parse(text: &str) -> Result<Expr, SyntaxError> {
    // 1. split the text into tokens
    // 2. run the grammar over the tokens
    // 3. on failure, pick the reason
    let input = Input::new(text, lex(text));
    let refusal = match rule(&input, 0) {
        Ok(Some((tree, _))) => return Ok(tree),
        Ok(None) => None,
        Err(refusal) => Some(refusal),
    };
    Err(explain(&input, refusal))
}

// Tokens.

/// The word `w` of the language, with its place.
fn word(w: &'static str) -> impl Parser<Range<usize>> {
    let is_word = move |t: &super::tokens::Token| {
        matches!(&t.kind, Tok::Name(n) if n == w).then(|| t.at.clone())
    };
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
    // "after `a > 1`".
    map(then_after(expr, end_of_rule), |(e, ())| e)(i, p)
}

/// `end := <the end of the text>`
///
/// Reads the end of `amount > 100`. In `a > 1) and b` it refuses the `)`.
fn end_of_rule(i: &Input<'_>, p: usize) -> Reply<()> {
    let end = token("the end of the rule", |t| {
        matches!(t.kind, Tok::End).then_some(())
    });
    let closer = token("", |t| match t.kind {
        Tok::Punct(s @ (")" | "]")) => s.chars().next(),
        _ => None,
    });
    // A closing bracket here closes nothing, because every opening bracket
    // consumes its own.
    let stray = try_map(closer, |_, close, at| {
        let reason = Reason::ClosesNothing { close };
        Err(SyntaxError { at, reason })
    });
    or(end, hidden(stray))(i, p)
}

/// `expr := conditional`
///
/// Reads any expression, such as `a or b`. It is what stands in parentheses,
/// in a list and in an argument.
fn expr(i: &Input<'_>, p: usize) -> Reply<Expr> {
    // An error says "an expression" and does not list every token that can
    // start one.
    label("an expression", conditional)(i, p)
}

/// `conditional := or_expr ("if" or_expr "else" conditional)?`
///
/// Reads `price if price > 0 else 0`. The branch after `else` may be a
/// conditional again, so `a if b else c if d else e` groups to the right.
fn conditional(i: &Input<'_>, p: usize) -> Reply<Expr> {
    // Python's `a if b` without `else` is the mistake to name.
    let needs_else = require(Reason::ConditionalNeedsElse, word("else"));
    // The branch after `else` is nested, because this rule calls itself
    // there.
    let tail = then(right(word("if"), or_expr), nested(needs_else, conditional));
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
    chain_left(and_expr, word("or"), |l, _, r| join(Expr::Or, l, r))(i, p)
}

/// `and_expr := not_expr ("and" not_expr)*`
///
/// Reads `a > 1 and b < 2 and c` as one `And` of three operands.
fn and_expr(i: &Input<'_>, p: usize) -> Reply<Expr> {
    chain_left(not_expr, word("and"), |l, _, r| join(Expr::And, l, r))(i, p)
}

/// Joins two operands of `and` or `or` into one node, and splices in an
/// operand that is a node of the same kind, so `(a and b) and c` is one list
/// of three.
fn join(make: fn(Vec<Expr>) -> Expr, l: Expr, r: Expr) -> Expr {
    let kind = mem::discriminant(&make(Vec::new()));
    let operands = |e: Expr| {
        let same = mem::discriminant(&e) == kind;
        match e {
            Expr::And(operands) | Expr::Or(operands) if same => operands,
            e => vec![e],
        }
    };
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
    // An error says "a value", and `not` is not listed beside it.
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
    choice!(
        // `==`, `!=`, `<=`, `>=`, `<` and `>`.
        symbol(&COMPARISONS),
        // `x in y`.
        map(word("in"), |_| Expr::In as Binary),
        // `x not in y`. A `not` after an operand can only start `not in`.
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
    // `> 100`. The places are kept for the errors of `build_comparison`.
    let operator_and_operand = then(spanned(compare_op), spanned(additive));
    let binary = map(operator_and_operand, |((make, op_at), (r, r_at))| {
        Tail::Binary(make, op_at, r, r_at)
    });
    // `is None` and `is not None`.
    let is_none = right(word("is"), then(opt(word("not")), word("None")));
    let is_none = map(is_none, |(not, _)| Tail::IsNone(not.is_some()));
    // An error says "an operator" and does not list all nine.
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
    // A second operator is read only to refuse the chain with the rewrite
    // that names its operands, and it is hidden from the expected set.
    let again = hidden(or(map(compare_op, |_| ()), map(word("is"), |_| ())));
    let chained = opt(then(spanned(again), opt(spanned(additive))));
    let tail = opt(then(compare_tail, chained));
    try_map(then(spanned(additive), tail), build_comparison)(i, p)
}

/// The node of a comparison from its parts, or why it is none: a chain of
/// comparisons, or a comparison with the literal `None`.
fn build_comparison(
    i: &Input<'_>,
    ((left, left_at), tail): (Placed<Expr>, Option<(Tail, Chained)>),
    _: Range<usize>,
) -> Result<Expr, SyntaxError> {
    let text = |at: &Range<usize>| i.slice(at);
    let Some((tail, chained)) = tail else {
        return Ok(left);
    };
    if let Some((((), again_at), third)) = chained {
        // The rewrite needs two comparisons of two operands each, so it is
        // left out after `is None` and when the third operand is missing.
        let write = match (&tail, third) {
            (Tail::Binary(_, op_at, _, right_at), Some((_, third_at))) => {
                let first = format!("{} {} {}", text(&left_at), text(op_at), text(right_at));
                let second = format!("{} {} {}", text(right_at), text(&again_at), text(&third_at));
                Some(format!("{first} and {second}"))
            }
            _ => None,
        };
        let reason = Reason::ChainedComparison { write };
        return Err(SyntaxError {
            at: again_at,
            reason,
        });
    }
    let (make, op_at, right, right_at) = match tail {
        Tail::IsNone(false) => return Ok(Expr::IsNone(Box::new(left))),
        Tail::IsNone(true) => return Ok(Expr::IsNotNone(Box::new(left))),
        Tail::Binary(make, op_at, right, right_at) => (make, op_at, right, right_at),
    };
    // A comparison with a missing value turns the rule off (ADR-0005), so
    // `x == None` could never do what its author meant. The refusal gives
    // the `is None` form of the same test, with `None` on either side.
    let none_test = match text(&op_at) {
        "==" => Some(("==", "is None")),
        "!=" => Some(("!=", "is not None")),
        _ => None,
    };
    let (left_none, right_none) = (matches!(left, Expr::None), matches!(right, Expr::None));
    if let Some((op, is)) = none_test.filter(|_| left_none || right_none) {
        let (at, operand) = if right_none {
            (op_at.start..right_at.end, &left_at)
        } else {
            (left_at.start..op_at.end, &right_at)
        };
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
    chain_left(term, op, |l, make, r| make(Box::new(l), Box::new(r)))(i, p)
}

/// `term := unary (("*" | "/" | "%") unary)*`
///
/// Reads `price * qty % 7`, grouped to the left as `(price * qty) % 7`.
fn term(i: &Input<'_>, p: usize) -> Reply<Expr> {
    let op = hidden(symbol(&TERM));
    chain_left(unary, op, |l, make, r| make(Box::new(l), Box::new(r)))(i, p)
}

/// `unary := "-" number | "-" unary | postfix`
///
/// Reads a leading minus. On a number token it gives a negative literal, as
/// in `-5`, and on anything else a `Neg`, as in `-a` and `-(5)`.
fn unary(i: &Input<'_>, p: usize) -> Reply<Expr> {
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
        // smallest integer can be written.
        try_map(right(punct("-"), hidden(int)), |_, v, at| {
            let reason = Reason::IntTooLarge;
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
}

/// `postfix := atom ("." name | "[" index "]" | "[" text "]")*`
///
/// Reads `items[0].price` and `flows["in"]`.
fn postfix(i: &Input<'_>, p: usize) -> Reply<Expr> {
    // The steps are hidden from expected sets, because a `.` or a `[` may
    // follow almost any value.
    let steps = many(spanned(hidden(or(dot_step, bracket_step))));
    try_map(then(spanned(atom), steps), apply_postfixes)(i, p)
}

/// `"." name`
///
/// Reads the `.price` of `item.price`.
fn dot_step(i: &Input<'_>, p: usize) -> Reply<Post> {
    // Any name token is read here, also a word of the language, so that
    // `flows.in` gets a reason of its own in `apply_postfixes`.
    map(right(punct("."), spanned(any_name)), |(name, at)| {
        if WORDS.contains(&name.as_str()) {
            // `.in`: a word of the language is never a key.
            Post::Word(name, at)
        } else {
            // `.price`: a key.
            Post::Step(Segment::Key(name))
        }
    })(i, p)
}

/// `"[" (index | text) "]"`
///
/// Reads the `[0]` of `items[0]` and the `["in"]` of `flows["in"]`.
fn bracket_step(i: &Input<'_>, p: usize) -> Reply<Post> {
    let step = token("an index or a key", |t| match &t.kind {
        Tok::Int(v) => u32::try_from(*v).ok().map(Segment::Index),
        Tok::Text(s) => Some(Segment::Key(s.clone())),
        _ => None,
    });
    let number = token("a number", |t| matches!(t.kind, Tok::Int(_)).then_some(()));
    // `[-1]`. Python's index from the end is refused by name (ADR-0041).
    let negative = try_map(then(punct("-"), number), |_, _, at| {
        let reason = Reason::NegativeIndex;
        Err::<Segment, _>(SyntaxError { at, reason })
    });
    let inside = left(or(step, hidden(negative)), punct("]"));
    map(right(punct("["), inside), Post::Step)(i, p)
}

/// Applies the postfix steps to the atom, left to right.
fn apply_postfixes(
    i: &Input<'_>,
    ((atom, atom_at), posts): (Placed<Expr>, Vec<Placed<Post>>),
    _: Range<usize>,
) -> Result<Expr, SyntaxError> {
    let refuse = |at: Range<usize>, reason| Err(SyntaxError { at, reason });
    let bare_record = |e: &Expr| matches!(e, Expr::Field(path) if path.is_empty());
    let mut e = atom;
    for (post, at) in posts {
        e = match (post, e) {
            // `record[0]`: the record is an object, so its first step is a
            // key.
            (Post::Step(Segment::Index(_)), e) if bare_record(&e) => {
                return refuse(atom_at.start..at.end, Reason::BareRecord);
            }
            (Post::Step(step), Expr::Field(mut path)) => {
                path.push(step);
                Expr::Field(path)
            }
            // Only a path has keys.
            (Post::Step(_), _) => return refuse(at, Reason::KeyOnValue),
            (Post::Word(word, word_at), _) => {
                // The bracket form of the same key, built from the text
                // before the dot.
                let before = i.slice(&(atom_at.start..at.start)).trim_end();
                let write = format!("{before}[\"{word}\"]");
                return refuse(word_at, Reason::WordAfterDot { word, write });
            }
        };
    }
    // `record` with no key is the whole record, which is no value.
    if bare_record(&e) {
        return refuse(atom_at, Reason::BareRecord);
    }
    Ok(e)
}

// Atoms.

/// `atom := "record" | name | literal | "(" items ")" | "[" items "]"`
///
/// Reads `record`, `amount`, `100`, `(a or b)` and `[1, 2]`.
fn atom(i: &Input<'_>, p: usize) -> Reply<Expr> {
    choice!(
        // `record`, the start of `record["unit price"]`.
        map(word("record"), |_| Expr::Field(Vec::new())),
        // `amount`. A bare name is a field of the record.
        map(name, |name| Expr::Field(vec![Segment::Key(name)])),
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
        // minus before it.
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
    let text = || {
        token("a literal", |t| match &t.kind {
            Tok::Text(s) => Some(s.clone()),
            _ => None,
        })
    };
    let second = opt(hidden(spanned(text())));
    try_map(then(text(), second), |_, (value, second), _| match second {
        None => Ok(Expr::Text(value)),
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
    let some = then(then(expr, many(right(punct(","), expr))), opt(punct(",")));
    map(opt(some), |items| match items {
        // `()`.
        None => (Vec::new(), false),
        // `a, b` and `a,`.
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
    let inside = delimited(punct("("), items, punct(")"));
    map(inside, |(mut items, comma)| match items.pop() {
        // `(a)`: one item and no comma is the item itself.
        Some(only) if items.is_empty() && !comma => only,
        Some(last) => {
            items.push(last);
            Expr::List(items)
        }
        None => Expr::List(items),
    })(i, p)
}

/// `"[" items "]"`
///
/// Reads the list `[1, 2]`.
fn bracket_list(i: &Input<'_>, p: usize) -> Reply<Expr> {
    let inside = delimited(punct("["), items, punct("]"));
    map(inside, |(items, _)| Expr::List(items))(i, p)
}
