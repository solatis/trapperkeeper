//! A small kit of parser combinators over a list of tokens. A parser is a
//! function from the input and a token index. It gives a match with the next
//! index, no match, or a refusal. A choice tries its alternatives in order,
//! each from the same place.
//!
//! No reply carries a failure. A token test that does not match notes what
//! it wanted in the input's log, and the error is built from the notes
//! furthest in the text.
//!
//! One example runs through the comments of this file: the rule
//! `x not in (1, 2)` and the faulty `x not y`. The tokens of the rule are
//! `x`, `not`, `in`, `(`, `1`, `,`, `2`, `)` and the end, at the indexes 0
//! to 8. The tokens of the faulty text are `x`, `not`, `y` and the end, at
//! the indexes 0 to 3.

use std::cell::{Cell, RefCell};
use std::ops::Range;

use super::error::{Found, Reason, SyntaxError};
use super::tokens::{Tok, Token};
use crate::limits::{MAX_NESTING, MAX_PARSE_STEPS};

/// What the token tests wanted at the furthest token where one did not
/// match.
#[derive(Default)]
struct Log {
    // The token index of the notes. It only grows during a parse. In
    // `x not y` it ends at 2, the index of `y`.
    pos: usize,
    // What the tests at `pos` wanted, each once, in the order they ran.
    // These are the words of the error. In `x not y` it ends as "`in`".
    wanted: Vec<String>,
    // The notes that `label` and `hidden` took out of `wanted`. An error
    // does not list them, and they still say what could stand at the place.
    replaced: Vec<String>,
    // The text that the place comes after, when `then_after` recorded it.
    after: Option<Range<usize>>,
}

/// The tokens of one text, and what the parse has noted so far.
pub(super) struct Input<'t> {
    pub text: &'t str,
    pub tokens: Vec<Token>,
    // A `RefCell`, because a parser takes the input by shared reference.
    // Every borrow ends before a parser runs again, so no two borrows
    // meet.
    log: RefCell<Log>,
    // How deep the parse is nested right now. `nested` raises it and lowers
    // it again.
    nesting: Cell<usize>,
    // The token tests so far, held to `MAX_PARSE_STEPS`.
    steps: Cell<usize>,
}

impl<'t> Input<'t> {
    pub fn new(text: &'t str, tokens: Vec<Token>) -> Self {
        let (log, nesting, steps) = Default::default();
        Input {
            text,
            tokens,
            log,
            nesting,
            steps,
        }
    }

    /// The token at `p`. Past the end it is the last token, which is `End`
    /// or `Bad`.
    pub fn tok(&self, p: usize) -> &Token {
        // `lex` always ends the list with `End` or `Bad`, so the list is
        // never empty. An index past the end comes from a parser that
        // matched `End`, whose reply index is one past the last token.
        &self.tokens[p.min(self.tokens.len() - 1)]
    }

    /// The bytes of the tokens from `from` up to `to`, not including it.
    pub fn span(&self, from: usize, to: usize) -> Range<usize> {
        let start = self.tok(from).at.start;
        // The last token read is `to - 1`. In `x not in (1, 2)` the span
        // from 3 to 8 ends where the `)` at index 7 ends, at byte 15. No
        // tokens give the empty range where token `from` starts.
        let end = if to > from {
            self.tok(to - 1).at.end
        } else {
            start
        };
        start..end
    }

    pub fn slice(&self, at: &Range<usize>) -> &'t str {
        &self.text[at.clone()]
    }

    /// Notes that a test at token `p` wanted `what`. A note further in the
    /// text replaces the nearer ones, and a note at the same token joins
    /// them.
    pub fn note(&self, p: usize, what: &str) {
        let mut log = self.log.borrow_mut();
        // In `x not y` the tests after `x` note "an operator" and "`in`" at
        // index 1. Then the test for `in` after `not` fails at index 2. The
        // log moves there and holds "`in`" alone, and `replaced` and
        // `after` start empty again. No later test gets past index 1, so
        // the error is "expected `in` after `not`, found `y`". A note at
        // `pos` joins the others unless it is there already, and a note
        // behind `pos` is dropped.
        if p > log.pos {
            *log = Log {
                pos: p,
                wanted: vec![what.to_owned()],
                ..Log::default()
            };
        } else if p == log.pos && !log.wanted.iter().any(|w| w == what) {
            log.wanted.push(what.to_owned());
        }
    }

    /// Where the log stands, for `rename`.
    fn mark(&self) -> (usize, usize) {
        // The index of the notes and how many there are. In `x not y`,
        // `label("an operator")` around `compare_tail` starts at index 1
        // and gets (1, 0), because every note at index 1 so far was hidden.
        let log = self.log.borrow();
        (log.pos, log.wanted.len())
    }

    /// Replaces the notes made at `p` since `mark` by `name`, or by nothing.
    /// It does nothing when a note lies further than `p`.
    fn rename(&self, p: usize, mark: (usize, usize), name: Option<&str>) {
        let mut log = self.log.borrow_mut();
        // The body noted something past `p`, and those notes are about a
        // later token. In `x not y`, `label("an operator")` starts at index
        // 1 and the note "`in`" lies at index 2, so the note stays.
        if log.pos > p {
            return;
        }
        // The log moves to `p` also when nothing is left there, so that the
        // notes of an earlier token do not stand in for this place.
        if log.pos < p {
            *log = Log {
                pos: p,
                ..Log::default()
            };
        }
        // When the log stood at `p` before the combinator started, the
        // first `mark.1` notes there are older than its body, and they
        // stay. Otherwise every note at `p` is the body's. In `a adn b`
        // the hidden test for a closing bracket after `a` starts at index
        // 1 with the mark (1, 5), and only its own note leaves `wanted`.
        let kept = if mark.0 == p { mark.1 } else { 0 };
        // The body's notes leave the error and move to `replaced`, where
        // `furthest` still finds them.
        let taken = log.wanted.split_off(kept);
        log.replaced.extend(taken);
        // `label` adds its name, once. `hidden` passes no name.
        if let Some(name) = name.filter(|n| !log.wanted.iter().any(|w| w == n)) {
            log.wanted.push(name.to_owned());
        }
    }

    /// The token index of the furthest notes, and everything that a test
    /// wanted there, listed in an error or not.
    pub fn furthest(&self) -> (usize, Vec<String>) {
        // The grammar reads this to name a word that could stand at the
        // place. In `x not y` it gives index 2 and "`in`". `replaced` is
        // part of it, so `x iss None` is offered `is`, which
        // `label("an operator")` took out of `wanted`.
        let log = self.log.borrow();
        let all = log.wanted.iter().chain(&log.replaced);
        (log.pos, all.cloned().collect())
    }

    /// Records that the notes at `p` come after the text at `at`.
    fn note_after(&self, p: usize, at: Range<usize>) {
        let mut log = self.log.borrow_mut();
        // Notes further than `p` stand after other text. In `x not y` the
        // `then_after` of `rule` fails at index 1 and the notes lie at index
        // 2, so `after` stays empty, and `explain` takes the `not` before
        // `y`.
        if log.pos == p {
            log.after = Some(at);
        }
    }
}

/// An error that ends the parse, and the token index the parse had reached.
pub(super) struct Refusal {
    pub pos: usize,
    pub error: SyntaxError,
}

/// A match with the next token index, no match, or a refusal.
// `Ok(Some((value, next)))` is a match, `Ok(None)` is no match, and `Err`
// is a refusal, which every combinator passes on to its caller.
pub(super) type Reply<T> = Result<Option<(T, usize)>, Refusal>;

// Every function of this shape is a parser, the closures of the kit and the
// rules of the grammar alike, such as `fn rule(&Input, usize) -> Reply<Expr>`.
pub(super) trait Parser<T>: Fn(&Input<'_>, usize) -> Reply<T> {}
impl<T, F: Fn(&Input<'_>, usize) -> Reply<T>> Parser<T> for F {}

/// One token that `f` accepts, named `what` in errors. In `x not in (1, 2)`
/// the test for the word `not` matches the token at index 1 and replies with
/// index 2. In `x not y` the test for `in` meets `y` and notes "`in`".
pub(super) fn token<T>(what: impl Into<String>, f: impl Fn(&Token) -> Option<T>) -> impl Parser<T> {
    let what = what.into();
    move |i: &Input<'_>, p: usize| {
        // Past `MAX_PARSE_STEPS` the reply is the refusal `TooComplex`,
        // which `explain` reports over everything else. The count is a
        // `Cell`, because a parser takes the input by shared reference.
        let steps = i.steps.get() + 1;
        i.steps.set(steps);
        if steps > MAX_PARSE_STEPS {
            let (at, reason) = (0..i.text.len(), Reason::TooComplex);
            let error = SyntaxError { at, reason };
            return Err(Refusal { pos: p, error });
        }
        // A match reads one token, so its reply index is `p + 1`. No match
        // leaves the index to the caller, which tries its next alternative
        // from `p`.
        let reply = f(i.tok(p)).map(|v| (v, p + 1));
        // No other code makes a note.
        if reply.is_none() {
            i.note(p, &what);
        }
        Ok(reply)
    }
}

/// The value of `a` through `f`: `compare_op` turns the tokens `not in` of
/// `x not in (1, 2)` into the builder `Expr::NotIn`.
pub(super) fn map<T, U>(a: impl Parser<T>, f: impl Fn(T) -> U) -> impl Parser<U> {
    move |i: &Input<'_>, p: usize| Ok(a(i, p)?.map(|(x, q)| (f(x), q)))
}

/// Like `map`, and `f` may refuse the value with a reason. `f` also gets
/// the bytes the value came from. `comparison` refuses `x == None` this way:
/// the tokens matched and their meaning is wrong.
pub(super) fn try_map<T, U>(
    a: impl Parser<T>,
    f: impl Fn(&Input<'_>, T, Range<usize>) -> Result<U, SyntaxError>,
) -> impl Parser<U> {
    move |i: &Input<'_>, p: usize| {
        // When `a` does not match, `f` does not run.
        let Some((x, q)) = a(i, p)? else {
            return Ok(None);
        };
        // The refusal stands at `q`, the index after the tokens that `a`
        // read. In `x == None` that is index 3, the end.
        let value = f(i, x, i.span(p, q)).map_err(|error| Refusal { pos: q, error })?;
        Ok(Some((value, q)))
    }
}

/// The value of `a` with the bytes it came from: `(1, 2)` with bytes 9 to
/// 15 of `x not in (1, 2)`.
pub(super) fn spanned<T>(a: impl Parser<T>) -> impl Parser<(T, Range<usize>)> {
    move |i: &Input<'_>, p: usize| Ok(a(i, p)?.map(|(x, q)| ((x, i.span(p, q)), q)))
}

/// `a`, then `b`: `not` at index 1, then `in` at index 2, with the reply
/// index 3.
pub(super) fn then<A, B>(a: impl Parser<A>, b: impl Parser<B>) -> impl Parser<(A, B)> {
    move |i: &Input<'_>, p: usize| {
        let Some((x, q)) = a(i, p)? else {
            return Ok(None);
        };
        // When `b` does not match, the pair does not match, and the caller
        // starts again from `p`. In `x not y` the `not` matches, `in` does
        // not, and `compare_op` gets no match at index 1.
        Ok(b(i, q)?.map(|(y, r)| ((x, y), r)))
    }
}

/// Like `then`, and a note that `b` makes right after `a` says after which
/// text it stands, as in "after `a > 1`".
pub(super) fn then_after<A, B>(a: impl Parser<A>, b: impl Parser<B>) -> impl Parser<(A, B)> {
    move |i: &Input<'_>, p: usize| {
        let Some((first, mid)) = a(i, p)? else {
            return Ok(None);
        };
        // In `a > 1 b < 2` the `then_after` of `rule` reads `a > 1` at the
        // indexes 0 to 2. The end of the rule is missing at index 3, so the
        // notes there stand after the bytes of `a > 1`.
        let Some((second, end)) = b(i, mid)? else {
            i.note_after(mid, i.span(p, mid));
            return Ok(None);
        };
        Ok(Some(((first, second), end)))
    }
}

/// `a`, then `b`, keeping the value of `a`: `left(expr, punct(")"))` on the
/// tokens `2 )` of `(1, 2)` keeps the `2`.
pub(super) fn left<A, B>(a: impl Parser<A>, b: impl Parser<B>) -> impl Parser<A> {
    map(then(a, b), |(x, _)| x)
}

/// `a`, then `b`, keeping the value of `b`: `right(punct(","), expr)` on the
/// tokens `, 2` of `(1, 2)` keeps the `2`.
pub(super) fn right<A, B>(a: impl Parser<A>, b: impl Parser<B>) -> impl Parser<B> {
    map(then(a, b), |(_, y)| y)
}

/// `a`, or nothing: the comma that may follow the last item of `(1, 2)`.
pub(super) fn opt<T>(a: impl Parser<T>) -> impl Parser<Option<T>> {
    move |i: &Input<'_>, p: usize| {
        Ok(Some(match a(i, p)? {
            Some((x, q)) => (Some(x), q),
            // In `(1, 2)` no comma stands at the `)`, index 7, so `opt`
            // matches nothing there and replies with index 7. The note
            // "`,`" stays in the log.
            None => (None, p),
        }))
    }
}

/// `a` as often as it matches: the `, 2` after the first item of `(1, 2)`.
pub(super) fn many<T>(a: impl Parser<T>) -> impl Parser<Vec<T>> {
    move |i: &Input<'_>, p: usize| {
        // `many` always matches, with an empty list when `a` does not match
        // at `p`.
        let (mut values, mut pos) = (Vec::new(), p);
        // In `(1, 2)` the first round reads `, 2` at the indexes 5 and 6,
        // and the second meets `)` at index 7 and stops. A match that
        // consumed nothing would match again at the same index forever, so
        // `many` stops there and drops that value.
        while let Some((x, q)) = a(i, pos)?.filter(|(_, q)| *q > pos) {
            values.push(x);
            pos = q;
        }
        Ok(Some((values, pos)))
    }
}

/// `a`, and when it does not match, `b` from the same place: after `x`, the
/// operator `in` or else `not in`.
pub(super) fn or<T>(a: impl Parser<T>, b: impl Parser<T>) -> impl Parser<T> {
    // A refusal from `a` ends the choice, and `b` does not run. In
    // `lenn(name) > 3` the refusal of `call` keeps `atom` from reading
    // `lenn` as a field.
    move |i: &Input<'_>, p: usize| match a(i, p)? {
        // `b` starts at `p`, however far `a` read before it failed. In
        // `x not in (1, 2)` the `in` fails at index 1, and `not in` reads
        // from index 1 again.
        None => b(i, p),
        matched => Ok(matched),
    }
}

/// `choice!(a, b, c)` is `or(a, or(b, c))`.
macro_rules! choice {
    ($a:expr $(,)?) => { $a };
    ($a:expr, $($rest:expr),+ $(,)?) => {
        $crate::text::kit::or($a, $crate::text::kit::choice!($($rest),+))
    };
}
pub(super) use choice;

/// `x (op x)*`, folded to the left with `f`: `a - b - c` as `(a - b) - c`.
/// An operator with no operand after it is left where it stands.
pub(super) fn chain_left<T, O>(
    x: impl Parser<T>,
    op: impl Parser<O>,
    f: impl Fn(T, O, T) -> T,
) -> impl Parser<T> {
    move |i: &Input<'_>, p: usize| {
        let Some((mut folded, mut pos)) = x(i, p)? else {
            return Ok(None);
        };
        // Each round reads an operator and the operand after it, and folds
        // both into the value so far: `a - b`, then `(a - b) - c`.
        while let Some((o, q)) = op(i, pos)? {
            // In `price * > 3` the `*` matches and no operand follows it.
            // The reply ends before the `*`, and the note "a value" at `>`
            // lies furthest and becomes the error.
            let Some((right, r)) = x(i, q)? else {
                break;
            };
            folded = f(folded, o, right);
            pos = r;
        }
        Ok(Some((folded, pos)))
    }
}

/// Names `a` in errors: when `a` does not match and noted nothing further
/// than its start, its notes there are replaced by `name`. So an operand is
/// "a value" and not a list of every token that can start one.
pub(super) fn label<T>(name: &'static str, a: impl Parser<T>) -> impl Parser<T> {
    move |i: &Input<'_>, p: usize| {
        // In `amount >`, `unary` is labelled "a value" and starts at index
        // 2, the end. Its mark is (1, 0), because the notes at index 1 were
        // all hidden.
        let mark = i.mark();
        // The body fails at index 2 and notes there what can start a
        // value: "`-`", "a name", "`record`", "`None`", "a literal", "`(`"
        // and more.
        let reply = a(i, p)?;
        // `rename` moves those notes to `replaced` and leaves "a value" in
        // `wanted`. The error reads "expected a value after `>`, found the
        // end of the rule".
        if reply.is_none() {
            i.rename(p, mark, Some(name));
        }
        Ok(reply)
    }
}

/// Keeps `a` out of errors: when `a` does not match where it started, its
/// notes there are dropped. For a token that may follow almost any value,
/// such as `+`.
pub(super) fn hidden<T>(a: impl Parser<T>) -> impl Parser<T> {
    move |i: &Input<'_>, p: usize| {
        // In `a > 1 b < 2`, `term` hides its operator test at index 3, the
        // `b`. The mark is (3, 0), because the steps of `postfix` at index
        // 3 were hidden too.
        let mark = i.mark();
        // The test notes "an operator" at index 3.
        let reply = a(i, p)?;
        // `rename` moves that note to `replaced`. Every operator test after
        // `1` is hidden this way, so the error lists `and`, `or`, `if` and
        // the end of the rule, and no operator.
        if reply.is_none() {
            i.rename(p, mark, None);
        }
        Ok(reply)
    }
}

/// `a`, and when it does not match, the refusal `reason` at the token found
/// there. For a keyword whose absence is a mistake with a name, such as the
/// `else` of a conditional.
pub(super) fn require<T>(reason: Reason, a: impl Parser<T>) -> impl Parser<T> {
    move |i: &Input<'_>, p: usize| {
        let reply = a(i, p)?;
        // A bad token there is no refusal. The tokenizer stopped at it, so
        // the keyword may stand after it, as the `else` of `a if b; else c`
        // does, and `explain` reports the bad token.
        if reply.is_none() && !matches!(i.tok(p).kind, Tok::Bad(_)) {
            // In `price if price > 0` the `else` is missing at index 5, the
            // end, and the refusal points at bytes 18 to 18.
            let (at, reason) = (i.tok(p).at.clone(), reason.clone());
            let error = SyntaxError { at, reason };
            return Err(Refusal { pos: p, error });
        }
        Ok(reply)
    }
}

/// `open`, then `body` one level of nesting deeper: `not`, then its
/// operand. Opening a level past `MAX_NESTING` is refused at the opening
/// token.
pub(super) fn nested<O, T>(open: impl Parser<O>, body: impl Parser<T>) -> impl Parser<T> {
    move |i: &Input<'_>, p: usize| {
        let Some((_, q)) = open(i, p)? else {
            return Ok(None);
        };
        // The check runs before the body, because the body is where the
        // parser calls itself. In a text that opens 33 parentheses, the
        // 33rd sees the level 32 and is refused at byte 32.
        let level = i.nesting.get();
        if level >= MAX_NESTING {
            let (at, reason) = (i.tok(p).at.clone(), Reason::TooDeep { max: MAX_NESTING });
            let error = SyntaxError { at, reason };
            return Err(Refusal { pos: p, error });
        }
        // The body runs one level deeper. In `not not x` the second `not`
        // sees the level 1.
        i.nesting.set(level + 1);
        let reply = body(i, q);
        // The level closes whether the body matched or not, because a choice
        // may read the same tokens again. A refusal from the body closes it
        // too, since `reply` holds it and no `?` returns early.
        i.nesting.set(level);
        reply
    }
}

/// `open body close`, one level deeper: `(1, 2)`. `open` is one token. When
/// the body matched and the text ends where `close` should stand, the
/// refusal points at the opening token.
pub(super) fn delimited<O, T, C>(
    open: impl Parser<O>,
    body: impl Parser<T>,
    close: impl Parser<C>,
) -> impl Parser<T> {
    // `closed` reads the body and the closing token. `nested` calls it with
    // the index after `open`: index 4 for the `(` of `x not in (1, 2)`.
    let closed = move |i: &Input<'_>, p: usize| {
        // In `(1, 2)` the body reads `1, 2`, the indexes 4 to 6.
        let Some((x, q)) = body(i, p)? else {
            return Ok(None);
        };
        // The `)` at index 7 closes it, and the reply index is 8.
        if let Some((_, r)) = close(i, q)? {
            return Ok(Some((x, r)));
        }
        // Only at the end of the text is the bracket "never closed". Any
        // other token there leaves an ordinary note, as the `)` of
        // `[1, 2) == x` does.
        if !matches!(i.tok(q).kind, Tok::End) {
            return Ok(None);
        }
        // `p` is the index after `open`, and `open` is one token, so `p - 1`
        // is the opening token. In `x in ("a", "b"` it is the `(` at bytes 5
        // to 6.
        let at = i.tok(p - 1).at.clone();
        // The opening token is `(` or `[`, and its one character names it.
        let open = i.slice(&at).chars().next().unwrap_or_default();
        let reason = Reason::NeverClosed { open };
        let error = SyntaxError { at, reason };
        // The refusal stands at the end, as the note of `close` does, so
        // `explain` reports the refusal.
        Err(Refusal { pos: q, error })
    };
    nested(open, closed)
}

/// The error the author gets, from the refusal, if any, and the log.
pub(super) fn explain(i: &Input<'_>, refusal: Option<Refusal>) -> SyntaxError {
    // 1. a refusal, unless a note lies further in the text
    // 2. a bad token at the furthest note
    // 3. the furthest notes, as "expected ..., found ..."

    // The parse is over, so nothing writes the log while `explain` reads it.
    let log = i.log.borrow();

    // 1. In `record.` the refusal that `record` is the whole record stands
    //    at `record`, and the note "a name" stands after the dot. The note
    //    is further, and it is the mistake. In `lenn(name) > 3` no note
    //    lies past the refusal at index 1, so the refusal is the error.
    //    `TooComplex` wins over any note, because the parse stopped there.
    if let Some(Refusal { pos, error }) = refusal
        && (pos >= log.pos || error.reason == Reason::TooComplex)
    {
        return error;
    }
    // 2. The error points at the token of the furthest notes. When that
    //    token is bad, the text could not be read there at all, so the
    //    tokenizer's reason says more than a list of what was wanted. In
    //    `amount > 100;` the reason is the unexpected `;`.
    let token = i.tok(log.pos);
    let at = token.at.clone();
    let found = match &token.kind {
        Tok::Bad(reason) => {
            let reason = reason.clone();
            return SyntaxError { at, reason };
        }
        // `amount >` ends where a value was wanted.
        Tok::End => Found::End,
        // In `x not y` the token is `y`.
        _ => Found::Token(i.slice(&at).to_owned()),
    };
    // 3. With no text recorded by `then_after`, the token before the place
    //    stands in for it. In `x not y` that is `not`. The first token,
    //    as in `in > 5`, has no token before it and gets no "after".
    let before = || (log.pos > 0).then(|| i.tok(log.pos - 1).at.clone());
    let after = log.after.clone().or_else(before);
    // `with_words` in the grammar fills `nearest` when a word is near.
    let reason = Reason::Expected {
        expected: log.wanted.clone(),
        found,
        after: after.map(|at| i.slice(&at).to_owned()),
        nearest: None,
    };
    SyntaxError { at, reason }
}
