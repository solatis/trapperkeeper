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
}

/// One syntax error: the bytes of the text it is about, and why.
#[derive(Debug, Clone, PartialEq)]
pub struct SyntaxError {
    pub at: Range<usize>,
    pub reason: Reason,
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
        }
    }
}

impl fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.reason.fmt(f)
    }
}

impl std::error::Error for SyntaxError {}
