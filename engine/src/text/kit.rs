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
//! `x not in (1, 2)` and the faulty `x not y`.

use std::cell::{Cell, RefCell};
use std::ops::Range;

use super::error::{Found, Reason, SyntaxError};
use super::tokens::{Tok, Token};
use crate::limits::{MAX_NESTING, MAX_PARSE_STEPS};

/// What the token tests wanted at the furthest token where one did not
/// match.
#[derive(Default)]
struct Log {
    pos: usize,
    wanted: Vec<String>,
    // The text that the place comes after, when `then_after` recorded it.
    after: Option<Range<usize>>,
}

/// The tokens of one text, and what the parse has noted so far.
pub(super) struct Input<'t> {
    pub text: &'t str,
    pub tokens: Vec<Token>,
    // A `RefCell`, because a parser takes the input by shared reference.
    log: RefCell<Log>,
    // How deep the parse is nested right now.
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
        &self.tokens[p.min(self.tokens.len() - 1)]
    }

    /// The bytes of the tokens from `from` up to `to`, not including it.
    pub fn span(&self, from: usize, to: usize) -> Range<usize> {
        let start = self.tok(from).at.start;
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
        // In `x not y` the test for `in` fails at `y`, further than anything
        // else, so the error is "expected `in`, found `y`".
        if p > log.pos {
            *log = Log {
                pos: p,
                wanted: vec![what.to_owned()],
                after: None,
            };
        } else if p == log.pos && !log.wanted.iter().any(|w| w == what) {
            log.wanted.push(what.to_owned());
        }
    }

    /// Where the log stands, for `rename`.
    fn mark(&self) -> (usize, usize) {
        let log = self.log.borrow();
        (log.pos, log.wanted.len())
    }

    /// Replaces the notes made at `p` since `mark` by `name`, or by nothing.
    /// It does nothing when a note lies further than `p`.
    fn rename(&self, p: usize, mark: (usize, usize), name: Option<&str>) {
        let mut log = self.log.borrow_mut();
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
        log.wanted.truncate(if mark.0 == p { mark.1 } else { 0 });
        if let Some(name) = name.filter(|n| !log.wanted.iter().any(|w| w == n)) {
            log.wanted.push(name.to_owned());
        }
    }

    /// Records that the notes at `p` come after the text at `at`.
    fn note_after(&self, p: usize, at: Range<usize>) {
        let mut log = self.log.borrow_mut();
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
pub(super) type Reply<T> = Result<Option<(T, usize)>, Refusal>;

pub(super) trait Parser<T>: Fn(&Input<'_>, usize) -> Reply<T> {}
impl<T, F: Fn(&Input<'_>, usize) -> Reply<T>> Parser<T> for F {}

/// One token that `f` accepts, named `what` in errors. In `x not in (1, 2)`
/// the test for the word `not` matches the second token.
pub(super) fn token<T>(what: impl Into<String>, f: impl Fn(&Token) -> Option<T>) -> impl Parser<T> {
    let what = what.into();
    move |i: &Input<'_>, p: usize| {
        // Past `MAX_PARSE_STEPS` the reply is the refusal `TooComplex`,
        // which `explain` reports over everything else.
        let steps = i.steps.get() + 1;
        i.steps.set(steps);
        if steps > MAX_PARSE_STEPS {
            let (at, reason) = (0..i.text.len(), Reason::TooComplex);
            let error = SyntaxError { at, reason };
            return Err(Refusal { pos: p, error });
        }
        let reply = f(i.tok(p)).map(|v| (v, p + 1));
        // No other code makes a note.
        if reply.is_none() {
            i.note(p, &what);
        }
        Ok(reply)
    }
}

/// The value of `a` through `f`: the token `1` as the integer 1.
pub(super) fn map<T, U>(a: impl Parser<T>, f: impl Fn(T) -> U) -> impl Parser<U> {
    move |i: &Input<'_>, p: usize| Ok(a(i, p)?.map(|(x, q)| (f(x), q)))
}

/// Like `map`, and `f` may refuse the value with a reason. `f` also gets
/// the bytes the value came from. An unknown function is refused this way:
/// the tokens matched and their meaning is wrong.
pub(super) fn try_map<T, U>(
    a: impl Parser<T>,
    f: impl Fn(&Input<'_>, T, Range<usize>) -> Result<U, SyntaxError>,
) -> impl Parser<U> {
    move |i: &Input<'_>, p: usize| {
        let Some((x, q)) = a(i, p)? else {
            return Ok(None);
        };
        let value = f(i, x, i.span(p, q)).map_err(|error| Refusal { pos: q, error })?;
        Ok(Some((value, q)))
    }
}

/// The value of `a` with the bytes it came from: `(1, 2)` with bytes 9 to
/// 15 of `x not in (1, 2)`.
pub(super) fn spanned<T>(a: impl Parser<T>) -> impl Parser<(T, Range<usize>)> {
    move |i: &Input<'_>, p: usize| Ok(a(i, p)?.map(|(x, q)| ((x, i.span(p, q)), q)))
}

/// `a`, then `b`: `not`, then `in`.
pub(super) fn then<A, B>(a: impl Parser<A>, b: impl Parser<B>) -> impl Parser<(A, B)> {
    move |i: &Input<'_>, p: usize| {
        let Some((x, q)) = a(i, p)? else {
            return Ok(None);
        };
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
        let Some((second, end)) = b(i, mid)? else {
            i.note_after(mid, i.span(p, mid));
            return Ok(None);
        };
        Ok(Some(((first, second), end)))
    }
}

/// `a`, then `b`, keeping the value of `a`: a list item before its comma.
pub(super) fn left<A, B>(a: impl Parser<A>, b: impl Parser<B>) -> impl Parser<A> {
    map(then(a, b), |(x, _)| x)
}

/// `a`, then `b`, keeping the value of `b`: the list after `in`.
pub(super) fn right<A, B>(a: impl Parser<A>, b: impl Parser<B>) -> impl Parser<B> {
    map(then(a, b), |(_, y)| y)
}

/// `a`, or nothing: the `not` that may stand before `in`.
pub(super) fn opt<T>(a: impl Parser<T>) -> impl Parser<Option<T>> {
    move |i: &Input<'_>, p: usize| {
        Ok(Some(match a(i, p)? {
            Some((x, q)) => (Some(x), q),
            None => (None, p),
        }))
    }
}

/// `a` as often as it matches: the `, 2` after the first item of `(1, 2)`.
pub(super) fn many<T>(a: impl Parser<T>) -> impl Parser<Vec<T>> {
    move |i: &Input<'_>, p: usize| {
        let (mut values, mut pos) = (Vec::new(), p);
        // A match that consumed nothing would match again forever.
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
    move |i: &Input<'_>, p: usize| match a(i, p)? {
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
        while let Some((o, q)) = op(i, pos)? {
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
        let mark = i.mark();
        let reply = a(i, p)?;
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
        let mark = i.mark();
        let reply = a(i, p)?;
        if reply.is_none() {
            i.rename(p, mark, None);
        }
        Ok(reply)
    }
}

/// `a`, and when it does not match, the refusal `reason` at the token found
/// there. For a keyword whose absence is a mistake with a name, such as the
/// `else` of a conditional.
#[expect(dead_code, reason = "the conditional that needs it is not parsed yet")]
pub(super) fn require<T>(reason: Reason, a: impl Parser<T>) -> impl Parser<T> {
    move |i: &Input<'_>, p: usize| {
        let reply = a(i, p)?;
        // A bad token there is no refusal, because the tokenizer's error
        // comes first in the text.
        if reply.is_none() && !matches!(i.tok(p).kind, Tok::Bad(_)) {
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
        // parser calls itself.
        let level = i.nesting.get();
        if level >= MAX_NESTING {
            let (at, reason) = (i.tok(p).at.clone(), Reason::TooDeep { max: MAX_NESTING });
            let error = SyntaxError { at, reason };
            return Err(Refusal { pos: p, error });
        }
        i.nesting.set(level + 1);
        let reply = body(i, q);
        // The level closes whether the body matched or not, because a choice
        // may read the same tokens again.
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
    let closed = move |i: &Input<'_>, p: usize| {
        let Some((x, q)) = body(i, p)? else {
            return Ok(None);
        };
        if let Some((_, r)) = close(i, q)? {
            return Ok(Some((x, r)));
        }
        // Only at the end of the text is the bracket "never closed". Any
        // other token there leaves an ordinary note.
        if !matches!(i.tok(q).kind, Tok::End) {
            return Ok(None);
        }
        let at = i.tok(p - 1).at.clone();
        let open = i.slice(&at).chars().next().unwrap_or_default();
        let reason = Reason::NeverClosed { open };
        let error = SyntaxError { at, reason };
        Err(Refusal { pos: q, error })
    };
    nested(open, closed)
}

/// The error the author gets, from the refusal, if any, and the log.
pub(super) fn explain(i: &Input<'_>, refusal: Option<Refusal>) -> SyntaxError {
    // 1. a refusal, unless a note lies further in the text
    // 2. a bad token at the furthest note
    // 3. the furthest notes, as "expected ..., found ..."
    let log = i.log.borrow();

    // 1. In `record.` the refusal that `record` is the whole record stands
    //    at `record`, and the note "a name" stands after the dot. The note
    //    is further, and it is the mistake.
    if let Some(Refusal { pos, error }) = refusal
        && (pos >= log.pos || error.reason == Reason::TooComplex)
    {
        return error;
    }
    let token = i.tok(log.pos);
    let at = token.at.clone();
    // 2. The text could not be read there at all, so the tokenizer's reason
    //    says more than a list of what was wanted.
    let found = match &token.kind {
        Tok::Bad(reason) => {
            let reason = reason.clone();
            return SyntaxError { at, reason };
        }
        Tok::End => Found::End,
        _ => Found::Token(i.slice(&at).to_owned()),
    };
    // 3. With no text recorded by `then_after`, the token before the place
    //    stands in for it.
    let before = || (log.pos > 0).then(|| i.tok(log.pos - 1).at.clone());
    let after = log.after.clone().or_else(before);
    let reason = Reason::Expected {
        expected: log.wanted.clone(),
        found,
        after: after.map(|at| i.slice(&at).to_owned()),
    };
    SyntaxError { at, reason }
}
