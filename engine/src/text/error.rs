//! What the parser returns for a text that is no rule: the place and a
//! reason from a closed list.

use std::fmt;
use std::ops::Range;

use super::grammar::{FUNCTIONS, METHODS, TYPES, names};

/// Why a text is no rule. Each case holds what its sentence needs, and
/// `Display` writes the sentence. A sentence names the mistake first. Where
/// a reason knows what the author of the rule should write instead, its
/// sentence says that next, as the text that the author would type.
#[derive(Debug, Clone, PartialEq)]
pub enum Reason {
    // Found by the tokenizer.
    /// A text of more than `MAX_TEXT_LENGTH` characters.
    TooLong { max: usize },
    /// The `;` of `amount > 100;`, a character that starts no token.
    UnexpectedCharacter { found: char },
    /// An operator of another language and ours for it, as `&&` and `and`.
    ForeignOperator {
        found: &'static str,
        write: &'static str,
    },
    /// The `# big ones` of `amount > 100 # big ones`.
    Comment,
    /// The `"abc` of `name == "abc`.
    UnclosedText,
    /// The `"""` of `"""abc""" == x`.
    TripleQuote,
    /// One or two of Python's prefix letters `r`, `b`, `u` and `f`, in
    /// either case, before the opening quote, as the `f` of `f"{a}"` or the
    /// `rb` of `rb"x"`. Every such prefix except `r` and `R` is refused.
    TextPrefix { prefix: String },
    /// The `\q` of `name == 'a\qb'`, and the `\u12` of `name == "\u12"`,
    /// which has too few digits.
    BadEscape { found: String },
    /// The `1_` of `amount > 1_`, and `0x` with no digits.
    MalformedNumber,
    /// The tokenizer refuses `99999999999999999999`, which no u64 holds. The
    /// grammar refuses `9223372036854775808` without a minus before it,
    /// which no i64 holds. The tokenizer reads the magnitude alone, and only
    /// the grammar knows whether a minus stands before it.
    IntTooLarge,
    /// The `1e999` of `amount > 1e999`, which no f64 holds.
    FloatTooLarge,

    // Found by the grammar.
    /// The empty text, or a text of spaces.
    Empty,
    /// What could have stood at the place, what stands there, and the text
    /// that the place comes after, which is the expression or the token
    /// before it. `nearest` is the word of the language nearest to the name
    /// at the place. It is chosen from every word that the parser could have
    /// read there. The error may not list that word, because it lists some
    /// words under one name, as "an operator" for `is` in `x iss None`.
    /// `nearest` is set only when the word is near enough to be a typo, and
    /// it is not set when the name is itself a word of the language.
    Expected {
        expected: Vec<String>,
        found: Found,
        after: Option<String>,
        nearest: Option<&'static str>,
    },
    /// A text that opens more than `MAX_NESTING` levels, such as 33
    /// parentheses around `x`.
    TooDeep { max: usize },
    /// The parser gives up after `MAX_PARSE_STEPS` token tests. Only a
    /// grammar in which two alternatives read the same bracketed text
    /// reaches that cap, so no text an author writes should get this reason.
    TooComplex,
    /// The `(` of `(a > 1 and b < 2`, when the text ends inside it.
    NeverClosed { open: char },
    /// The `)` of `a > 1) and b`.
    ClosesNothing { close: char },
    /// A name that differs from a word of the language only in case, where
    /// that word could stand.
    KeywordCase {
        found: String,
        keyword: &'static str,
    },
    /// A Python keyword outside the language, when the parse fails right
    /// after it.
    NotInLanguage { word: String },
    /// A word of the language after a dot, as the `in` of `flows.in > 3`.
    /// `write` is the bracket form of the same key, `flows["in"]`.
    WordAfterDot { word: String, write: String },
    /// `record == 1` and `record[0] == 1`, where `record` stands for the
    /// whole record.
    BareRecord,
    /// `re` without a function after it. Its sentence also says how to name a
    /// field `re`.
    ModuleAlone { word: &'static str },
    /// The `.x` of `"abc".x == 1`, a key on a value that is no field.
    KeyOnValue,
    /// The `-1` of `items[-1].price > 5`.
    NegativeIndex,
    /// A second comparison operator after a comparison, as the second `<` of
    /// `0 < amount < 10`. `write` is the two comparisons joined with `and`,
    /// `0 < amount and amount < 10`. It is `None` when the first comparison
    /// is `is None`, which has no right operand to write twice, and when the
    /// third operand is missing, as in `a < b <`.
    ChainedComparison { write: Option<String> },
    /// The second text of `"abc" "def" == name`, which Python would join.
    AdjacentTexts,
    /// The `lenn` of `lenn(name) > 3`. `nearest` is the known name nearest
    /// to it, `len` here, when one is near enough to be a typo.
    UnknownFunction {
        name: String,
        nearest: Option<&'static str>,
    },
    /// The `startwith` of `name.startwith("a")`, with `nearest` as above.
    UnknownMethod {
        name: String,
        nearest: Option<&'static str>,
    },
    /// The `number` of `isinstance(price, number)`, with `nearest` as above.
    UnknownType {
        name: String,
        nearest: Option<&'static str>,
    },
    /// The `(a, b)` of `len(a, b) > 1`, and the `("x")` of
    /// `name.upper("x")`.
    WrongArgumentCount {
        name: &'static str,
        takes: usize,
        given: usize,
    },
    /// The `pattern` of `re.search(pattern, x)`, which is no text literal.
    PatternNotLiteral { function: &'static str },
    /// The `)` of `any(i.price > 100)`, where `for` should stand.
    QuantifierNeedsFor { function: &'static str },
    /// The end of `price if price > 0`, where `else` should stand.
    ConditionalNeedsElse,
}

/// What stands at the place of an `Expected` error.
#[derive(Debug, Clone, PartialEq)]
pub enum Found {
    /// A word of the language. Its sentence also says how to name a field of
    /// that name, as `record["in"]`.
    Word(&'static str),
    /// Any other token, as the `y` of `x not y`.
    Token(String),
    /// The end of the text, as in `amount >`.
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

/// One syntax error: the bytes of the text it is about, and why. `Display`
/// writes the sentence of the reason alone, and whoever shows the error works
/// out the line and the column from `at` (ADR-0042).
#[derive(Debug, Clone, PartialEq)]
pub struct SyntaxError {
    pub at: Range<usize>,
    pub reason: Reason,
}

/// Joins `items` into a list for a sentence, with ", " between them and
/// " or " before the last one, as "a, b or c". One item comes back
/// unchanged, and an empty slice gives an empty string.
fn one_of(items: &[String]) -> String {
    match items {
        // This arm never runs. The only caller, the `Expected` arm of
        // `Display for Reason`, writes no list when the list is empty. The
        // slice patterns must still cover the empty slice.
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
            // The sentence reads "expected a, b or c after `x`, found `y`".
            // Two hints of `with_words` in `grammar.rs` follow it: how to
            // name a field that is a word of the language, and the nearest
            // word.
            Reason::Expected {
                expected,
                found,
                after,
                nearest,
            } => {
                let after = after.as_ref().map(|a| format!(" after `{a}`"));
                let after = after.unwrap_or_default();
                // When the error lists nothing that could stand at the place,
                // the sentence names only what stands there. `hidden` in
                // `kit.rs` can take every test out of the list.
                if expected.is_empty() {
                    write!(f, "unexpected {found}{after}")?;
                } else {
                    write!(f, "expected {}{after}, found {found}", one_of(expected))?;
                }
                // For `a and or b` the sentence adds how to name a field
                // `or`, because `or` is a word of the language and `found` is
                // then `Found::Word`.
                if let Found::Word(word) = found {
                    write!(f, "; to name a field `{word}`, write `record[\"{word}\"]`")?;
                }
                // For `a adn b` the sentence adds "did you mean `and`?".
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
            Reason::ModuleAlone { word } => write!(
                f,
                "`{word}` names the pattern functions; to name a field `{word}`, \
                 write `record[\"{word}\"]`"
            ),
            Reason::KeyOnValue => write!(f, "only a field has keys and indexes"),
            Reason::NegativeIndex => write!(f, "a negative index is not supported"),
            Reason::ChainedComparison { write } => {
                // `0 < amount < 10` gets the rewrite, and `a < b <` has none.
                write!(f, "chained comparisons are not supported")?;
                write.iter().try_for_each(|w| write!(f, "; write `{w}`"))
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
