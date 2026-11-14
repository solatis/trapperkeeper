//! What the parser returns for a text that is no rule: the place and a
//! reason from a closed list.

use std::fmt;
use std::ops::Range;

use super::grammar::{FUNCTIONS, METHODS, TYPES, names};

/// Why a text is no rule. Each case holds what its sentence needs, and
/// `Display` writes the sentence.
#[derive(Debug, Clone, PartialEq)]
pub enum Reason {
    // Found by the tokenizer.
    // A text of more than `MAX_TEXT_LENGTH` characters.
    TooLong {
        max: usize,
    },
    // The `;` of `amount > 100;`, a character that starts no token.
    UnexpectedCharacter {
        found: char,
    },
    // An operator of another language and ours for it, as `&&` and `and`.
    ForeignOperator {
        found: &'static str,
        write: &'static str,
    },
    // The `# big ones` of `amount > 100 # big ones`.
    Comment,
    // The `"abc` of `name == "abc`.
    UnclosedText,
    // The `"""` of `"""abc""" == x`.
    TripleQuote,
    // One of Python's letters before the opening quote other than `r`, as in
    // an f-string.
    TextPrefix {
        prefix: String,
    },
    // The `\q` of `name == 'a\qb'`, and the `\u12` of `name == "\u12"`,
    // which has too few digits.
    BadEscape {
        found: String,
    },
    // The `1_` of `amount > 1_`, and `0x` with no digits.
    MalformedNumber,
    // The tokenizer refuses `99999999999999999999`, which no u64 holds. The
    // grammar refuses `9223372036854775808` without a minus before it,
    // which no i64 holds.
    IntTooLarge,
    // The `1e999` of `amount > 1e999`, which no f64 holds.
    FloatTooLarge,

    // Found by the grammar, generic.
    // The empty text, or a text of spaces.
    Empty,
    // What could have stood at the place, what stands there, and the text
    // before it. `nearest` is the expected word nearest to a name that
    // stands at the place, when it is near enough to be a typo.
    Expected {
        expected: Vec<String>,
        found: Found,
        after: Option<String>,
        nearest: Option<&'static str>,
    },
    // A text that opens more than `MAX_NESTING` levels, such as 33
    // parentheses around `x`.
    TooDeep {
        max: usize,
    },
    // The parser gave up at the cap on its work. No rule an author writes
    // reaches it.
    TooComplex,

    // Found by the grammar, targeted.
    // The `(` of `(a > 1 and b < 2`, when the text ends inside it.
    NeverClosed {
        open: char,
    },
    // The `)` of `a > 1) and b`.
    ClosesNothing {
        close: char,
    },
    // A name that differs from a word of the language only in case, where
    // that word could stand.
    KeywordCase {
        found: String,
        keyword: &'static str,
    },
    // A Python keyword outside the language, when the parse fails right
    // after it.
    NotInLanguage {
        word: String,
    },
    // `write` is the bracket form of the same key.
    WordAfterDot {
        word: String,
        write: String,
    },
    // `record == 1` and `record[0] == 1`, where `record` stands for the
    // whole record.
    BareRecord,
    // `re` or `math` without a function after it. Its sentence also says how
    // to name a field of that name.
    ModuleAlone {
        word: &'static str,
    },
    // The `.x` of `"abc".x == 1`, a key on a value that is no field.
    KeyOnValue,
    // The `-1` of `items[-1].price > 5`.
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
    // The second text of `"abc" "def" == name`, which Python would join.
    AdjacentTexts,
    // The `lenn` of `lenn(name) > 3`. `nearest` is the known name nearest
    // to it, `len` here, when one is near enough to be a typo.
    UnknownFunction {
        name: String,
        nearest: Option<&'static str>,
    },
    // The `startwith` of `name.startwith("a")`, with `nearest` as above.
    UnknownMethod {
        name: String,
        nearest: Option<&'static str>,
    },
    // The `number` of `isinstance(price, number)`, with `nearest` as above.
    UnknownType {
        name: String,
        nearest: Option<&'static str>,
    },
    // The `(a, b)` of `len(a, b) > 1`, and the `("x")` of
    // `name.upper("x")`.
    WrongArgumentCount {
        name: &'static str,
        takes: usize,
        given: usize,
    },
    // The `pattern` of `re.search(pattern, x)`, which is no text literal.
    PatternNotLiteral {
        function: &'static str,
    },
    // The `)` of `any(i.price > 100)`, where `for` should stand.
    QuantifierNeedsFor {
        function: &'static str,
    },
    // The end of `price if price > 0`, where `else` should stand.
    ConditionalNeedsElse,
}

/// What stands at the place of an `Expected` error.
#[derive(Debug, Clone, PartialEq)]
pub enum Found {
    // A word of the language. Its sentence also says how to name a field of
    // that name, as `record["in"]`.
    Word(&'static str),
    // Any other token, as the `y` of `x not y`.
    Token(String),
    // The end of the text, as in `amount >`.
    End,
}

impl fmt::Display for Found {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Found::Word(word) => write!(f, "`{word}`"),
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
    // `Display` writes no list when `names` is empty, so the first arm
    // gives nothing.
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [first @ .., last] => format!("{} or {last}", first.join(", ")),
    }
}

/// The sentence for a name outside a closed list: the nearest known name
/// when there is one, and the whole list otherwise.
fn unknown(
    f: &mut fmt::Formatter<'_>,
    what: &str,
    name: &str,
    nearest: Option<&str>,
    known: &[&str],
) -> fmt::Result {
    // "unknown function `lenn`; did you mean `len`?", and with no name
    // near enough, the whole list, as for `abs`.
    match nearest {
        Some(nearest) => write!(f, "unknown {what} `{name}`; did you mean `{nearest}`?"),
        None => write!(
            f,
            "unknown {what} `{name}`; the {what}s are {}",
            known.join(", ")
        ),
    }
}

impl fmt::Display for Reason {
    // A sentence names the mistake first and then what to write, in the
    // words of an author. `faulty` in `tests/text.rs` pins every sentence.
    #[expect(clippy::too_many_lines, reason = "one sentence per reason")]
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
            // "expected a, b or c after `x`, found `y`", then what to write
            // when the grammar knows it.
            Reason::Expected {
                expected,
                found,
                after,
                nearest,
            } => {
                let after = after.as_ref().map(|a| format!(" after `{a}`"));
                let after = after.unwrap_or_default();
                // With no note left at the place, as when `hidden` dropped
                // them all, the sentence names only what stands there.
                if expected.is_empty() {
                    write!(f, "unexpected {found}{after}")?;
                } else {
                    write!(f, "expected {}{after}, found {found}", one_of(expected))?;
                }
                // `a and or b` adds how to name a field `or`.
                if let Found::Word(word) = found {
                    write!(f, "; to name a field `{word}`, write `record[\"{word}\"]`")?;
                }
                // `a adn b` adds "did you mean `and`?".
                if let Some(nearest) = nearest {
                    write!(f, "; did you mean `{nearest}`?")?;
                }
                Ok(())
            }
            Reason::TooDeep { max } => write!(f, "a rule nests at most {max} levels deep"),
            Reason::TooComplex => write!(f, "this rule is too complex to parse"),
            Reason::NeverClosed { open } => write!(f, "this `{open}` is never closed"),
            Reason::ClosesNothing { close } => write!(f, "this `{close}` closes nothing"),
            // `none` is told to write `None` with a capital, and `AND` to
            // write `and` in lowercase.
            Reason::KeywordCase { keyword, .. } => {
                if keyword.starts_with(char::is_uppercase) {
                    write!(f, "write `{keyword}` with a capital letter")
                } else {
                    write!(f, "keywords are lowercase: write `{keyword}`")
                }
            }
            Reason::NotInLanguage { word } => {
                write!(f, "`{word}` is not part of the rule language")
            }
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
            Reason::ModuleAlone { word } => {
                // `re` names the pattern functions and `math` the math
                // functions.
                let functions = if *word == "re" { "pattern" } else { "math" };
                write!(
                    f,
                    "`{word}` names the {functions} functions; to name a field `{word}`, \
                     write `record[\"{word}\"]`"
                )
            }
            Reason::KeyOnValue => write!(f, "only a field has keys and indexes"),
            Reason::NegativeIndex => write!(f, "a negative index is not supported"),
            Reason::ChainedComparison { write } => {
                // `0 < amount < 10` gets the rewrite, and `a < b <` has none.
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
            Reason::UnknownFunction { name, nearest } => {
                unknown(f, "function", name, *nearest, &names(&FUNCTIONS))
            }
            Reason::UnknownMethod { name, nearest } => {
                unknown(f, "method", name, *nearest, &names(&METHODS))
            }
            Reason::UnknownType { name, nearest } => {
                unknown(f, "type", name, *nearest, &names(&TYPES))
            }
            // A method that takes no arguments, as `upper`, gets a sentence
            // without numbers.
            Reason::WrongArgumentCount { name, takes: 0, .. } => {
                write!(f, "`{name}` takes no arguments")
            }
            Reason::WrongArgumentCount { name, takes, given } => {
                // "`len` takes 1 argument, and 2 were given", and
                // "`isinstance` takes 2 arguments, and 1 was given".
                let arguments = if *takes == 1 { "argument" } else { "arguments" };
                let were = if *given == 1 { "was" } else { "were" };
                write!(
                    f,
                    "`{name}` takes {takes} {arguments}, and {given} {were} given"
                )
            }
            Reason::PatternNotLiteral { function } => write!(
                f,
                "the pattern of `{function}` must be a text literal, as in \
                 `{function}(r\"^a\", name)`"
            ),
            Reason::QuantifierNeedsFor { function } => {
                write!(
                    f,
                    "`{function}` needs `for <name> in <list>` after its test"
                )
            }
            Reason::ConditionalNeedsElse => {
                write!(f, "a conditional needs `else`; write `a if test else b`")
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
