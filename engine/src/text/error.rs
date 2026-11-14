//! What the parser returns for a text that is no rule: the place and a
//! reason from a closed list.

use std::fmt;
use std::ops::Range;

/// Why a text is no rule. Each case holds what its sentence needs, and
/// `Display` writes the sentence.
#[derive(Debug, Clone, PartialEq)]
pub enum Reason {
    // Found by the tokenizer.
    TooLong {
        max: usize,
    },
    UnexpectedCharacter {
        found: char,
    },
    // An operator of another language and ours for it, as `&&` and `and`.
    ForeignOperator {
        found: &'static str,
        write: &'static str,
    },
    Comment,
    UnclosedText,
    TripleQuote,
    // One of Python's letters before the opening quote other than `r`, as in
    // an f-string.
    TextPrefix {
        prefix: String,
    },
    BadEscape {
        found: String,
    },
    MalformedNumber,
    IntTooLarge,
    FloatTooLarge,

    // Found by the grammar, generic.
    Empty,
    // What could have stood at the place, what stands there, and the text
    // before it.
    Expected {
        expected: Vec<String>,
        found: Found,
        after: Option<String>,
    },
    TooDeep {
        max: usize,
    },
    // The parser gave up at the cap on its work. No rule an author writes
    // reaches it.
    TooComplex,

    // Found by the grammar, targeted.
    NeverClosed {
        open: char,
    },
    ClosesNothing {
        close: char,
    },
    // `write` is the bracket form of the same key.
    WordAfterDot {
        word: String,
        write: String,
    },
    BareRecord,
    KeyOnValue,
    NegativeIndex,
    // `write` is the two comparisons joined with `and`, when both operators
    // are comparisons of two operands.
    ChainedComparison {
        write: Option<String>,
    },
    // `op` is `==` or `!=`, and `write` is the `is None` form of the same
    // test.
    NoneComparison {
        op: &'static str,
        write: String,
    },
    AdjacentTexts,
}

/// What stands at the place of an `Expected` error.
#[derive(Debug, Clone, PartialEq)]
pub enum Found {
    Token(String),
    End,
}

impl fmt::Display for Found {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Found::Token(token) => write!(f, "`{token}`"),
            Found::End => write!(f, "the end of the rule"),
        }
    }
}

/// One syntax error: the bytes of the text it is about, and why.
#[derive(Debug, Clone, PartialEq)]
pub struct SyntaxError {
    pub at: Range<usize>,
    pub reason: Reason,
}

/// "a", "a or b", "a, b or c".
fn one_of(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [first @ .., last] => format!("{} or {last}", first.join(", ")),
    }
}

impl fmt::Display for Reason {
    // A sentence names the mistake first and then what to write, in the
    // words of an author.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Reason::TooLong { max } => write!(f, "a rule is at most {max} characters long"),
            Reason::UnexpectedCharacter { found } => write!(f, "unexpected character `{found}`"),
            Reason::ForeignOperator { found, write } => {
                write!(f, "`{found}` is not an operator here; write `{write}`")
            }
            Reason::Comment => write!(f, "comments are not supported in a rule"),
            Reason::UnclosedText => write!(f, "this text is never closed"),
            Reason::TripleQuote => {
                write!(
                    f,
                    "triple-quoted texts are not supported; write one pair of quotes"
                )
            }
            Reason::TextPrefix { prefix } => {
                write!(
                    f,
                    "the text prefix `{prefix}` is not supported; only `r` is"
                )
            }
            Reason::BadEscape { found } => write!(f, "unknown escape `{found}` in a text"),
            Reason::MalformedNumber => write!(f, "malformed number"),
            Reason::IntTooLarge => write!(f, "this number is too large for an integer"),
            Reason::FloatTooLarge => write!(f, "this number is too large for a float"),
            Reason::Empty => write!(f, "a rule is empty; write an expression"),
            // "expected a, b or c after `x`, found `y`".
            Reason::Expected {
                expected,
                found,
                after,
            } => {
                let after = after.as_ref().map(|a| format!(" after `{a}`"));
                let after = after.unwrap_or_default();
                if expected.is_empty() {
                    write!(f, "unexpected {found}{after}")
                } else {
                    write!(f, "expected {}{after}, found {found}", one_of(expected))
                }
            }
            Reason::TooDeep { max } => write!(f, "a rule nests at most {max} levels deep"),
            Reason::TooComplex => write!(f, "this rule is too complex to parse"),
            Reason::NeverClosed { open } => write!(f, "this `{open}` is never closed"),
            Reason::ClosesNothing { close } => write!(f, "this `{close}` closes nothing"),
            Reason::WordAfterDot { word, write } => {
                write!(
                    f,
                    "`{word}` is a word of the rule language; write `{write}`"
                )
            }
            Reason::BareRecord => write!(
                f,
                "`record` is the whole record; name a field, as in `record[\"key\"]`"
            ),
            Reason::KeyOnValue => write!(f, "only a field has keys and indexes"),
            Reason::NegativeIndex => write!(f, "a negative index is not supported"),
            Reason::ChainedComparison { write } => {
                write!(f, "chained comparisons are not supported")?;
                write.iter().try_for_each(|w| write!(f, "; write `{w}`"))
            }
            Reason::NoneComparison { op, write } => {
                write!(
                    f,
                    "`{op}` does not test for a missing value; write `{write}`"
                )
            }
            Reason::AdjacentTexts => {
                write!(f, "two texts side by side are not joined; write one text")
            }
        }
    }
}

impl fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.reason.fmt(f)
    }
}

impl std::error::Error for SyntaxError {}
