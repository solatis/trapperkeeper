//! The tokenizer: it splits a rule's text into names, literals and
//! punctuation, each with its place. It does not fail. A text it cannot
//! split ends in a token that holds the error.

use std::ops::Range;

use super::error::{Reason, SyntaxError};
use crate::limits::MAX_TEXT_LENGTH;

/// What a token is.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Tok {
    // A word of the language is a name here. The grammar tells them apart.
    Name(String),
    // The magnitude without a sign, so the grammar can fold a leading minus
    // into the smallest integer.
    Int(u64),
    Float(f64),
    Text(String),
    Punct(&'static str),
    End,
    // The last token of a list, when the text could not be split further.
    Bad(Reason),
}

/// A token and the bytes of the text it came from.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Token {
    pub kind: Tok,
    pub at: Range<usize>,
}

// The operators and punctuation of the language. A longer one stands before
// its prefix, so `<=` is not read as `<`.
const PUNCTS: [&str; 17] = [
    "==", "!=", "<=", ">=", "<", ">", "+", "-", "*", "/", "%", "(", ")", "[", "]", ",", ".",
];

// Operators of other languages and ours for each. `<>`, `&&` and `||` are
// tried before `PUNCTS` and `!` and `=` after it, so `<>` is not read as `<`
// and `!=` is not read as `!`.
const FOREIGN: [(&str, &str); 5] = [
    ("<>", "!="),
    ("&&", "and"),
    ("||", "or"),
    ("!", "not"),
    ("=", "=="),
];

/// Splits `text` into tokens. The list always ends with `End` or `Bad`.
#[expect(dead_code, reason = "the grammar that calls it is not written yet")]
pub(super) fn lex(text: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    // The check counts characters and runs first, so a text over the cap
    // costs no further work. The bad token covers the characters beyond it.
    if let Some((cut, _)) = text.char_indices().nth(MAX_TEXT_LENGTH) {
        let kind = Tok::Bad(Reason::TooLong {
            max: MAX_TEXT_LENGTH,
        });
        let at = cut..text.len();
        tokens.push(Token { kind, at });
        return tokens;
    }
    let mut at = 0;
    while let Some(c) = text[at..].chars().next() {
        // A line break is a space, so a rule may run over several lines.
        if c.is_whitespace() {
            at += c.len_utf8();
            continue;
        }
        match next_token(text, at) {
            Ok(token) => {
                at = token.at.end;
                tokens.push(token);
            }
            Err(SyntaxError { at, reason }) => {
                let kind = Tok::Bad(reason);
                tokens.push(Token { kind, at });
                return tokens;
            }
        }
    }
    let at = text.len()..text.len();
    tokens.push(Token { kind: Tok::End, at });
    tokens
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// The token that starts at byte `at`, or why none does.
fn next_token(text: &str, at: usize) -> Result<Token, SyntaxError> {
    let rest = &text[at..];
    let refuse = |len: usize, reason| {
        Err(SyntaxError {
            at: at..at + len,
            reason,
        })
    };
    let Some(c) = rest.chars().next() else {
        unreachable!("`lex` calls this at a character")
    };

    if c.is_ascii_alphabetic() || c == '_' {
        let end = at + rest.find(|c| !is_name_char(c)).unwrap_or(rest.len());
        let word = &text[at..end];
        // The prefixes are Python's letters. Any other word before a quote
        // is a name, so `x in"a"` reads as it does in Python.
        let quoted = text[end..].starts_with(['"', '\'']);
        let prefix = quoted && word.len() <= 2 && word.chars().all(|c| "rbufRBUF".contains(c));
        if !prefix {
            let kind = Tok::Name(word.to_owned());
            return Ok(Token { kind, at: at..end });
        }
        // `r` and `R` start a raw text, and any other prefix is refused. The
        // literal is read raw either way, so the refusal covers all of it.
        let literal = self::text(text, at, end, true)?;
        if word.eq_ignore_ascii_case("r") {
            return Ok(literal);
        }
        let prefix = word.to_owned();
        return refuse(literal.at.len(), Reason::TextPrefix { prefix });
    }
    // A `.` followed by a digit starts a float, as in `.5`.
    let digit_next = rest[c.len_utf8()..].starts_with(|c: char| c.is_ascii_digit());
    if c.is_ascii_digit() || (c == '.' && digit_next) {
        return number(text, at);
    }
    if c == '"' || c == '\'' {
        return self::text(text, at, at, false);
    }
    if c == '#' {
        // The error covers the comment to the end of its line.
        let len = rest.find('\n').unwrap_or(rest.len());
        return refuse(len, Reason::Comment);
    }
    let foreign = |long: bool| {
        let fits =
            |(found, _): &&(&str, &str)| rest.starts_with(found) && (found.len() == 2) == long;
        FOREIGN.iter().find(fits).map(|&(found, write)| {
            let reason = Reason::ForeignOperator { found, write };
            refuse(found.len(), reason)
        })
    };
    if let Some(refusal) = foreign(true) {
        return refusal;
    }
    if let Some(p) = PUNCTS.iter().find(|p| rest.starts_with(**p)) {
        let at = at..at + p.len();
        return Ok(Token {
            kind: Tok::Punct(p),
            at,
        });
    }
    if let Some(refusal) = foreign(false) {
        return refusal;
    }
    refuse(c.len_utf8(), Reason::UnexpectedCharacter { found: c })
}

/// A number that starts at `at`: a decimal or hexadecimal integer, or a
/// float.
fn number(text: &str, at: usize) -> Result<Token, SyntaxError> {
    let bytes = text.as_bytes();
    let refuse = |end: usize, reason| {
        Err(SyntaxError {
            at: at..end,
            reason,
        })
    };
    // The end of the run of digits and underscores that starts at `from`.
    let run = |from: usize, hex: bool| {
        let fits = |b: &&u8| {
            **b == b'_'
                || if hex {
                    b.is_ascii_hexdigit()
                } else {
                    b.is_ascii_digit()
                }
        };
        from + bytes[from..].iter().take_while(fits).count()
    };

    let hex = bytes[at] == b'0' && matches!(bytes.get(at + 1), Some(b'x' | b'X'));
    let mut end = run(if hex { at + 2 } else { at }, hex);
    let mut float = false;
    if !hex {
        if bytes.get(end) == Some(&b'.') {
            float = true;
            end = run(end + 1, false);
        }
        if matches!(bytes.get(end), Some(b'e' | b'E')) {
            let sign = usize::from(matches!(bytes.get(end + 1), Some(b'+' | b'-')));
            if bytes.get(end + 1 + sign).is_some_and(u8::is_ascii_digit) {
                float = true;
                end = run(end + 1 + sign, false);
            }
        }
    }

    // A name character right after the digits makes the whole word a
    // malformed number, so `1abc` is one error and not a number and a name.
    let word_end = end
        + text[end..]
            .find(|c| !is_name_char(c))
            .unwrap_or(text.len() - end);
    if word_end > end {
        return refuse(word_end, Reason::MalformedNumber);
    }
    let digits = &text[if hex { at + 2 } else { at }..end];
    // An underscore stands between two digits.
    let underscores_fit = digits
        .split(['.', 'e', 'E', '+', '-'])
        .all(|part| !part.starts_with('_') && !part.ends_with('_') && !part.contains("__"));
    // `0x` with no digit after it is a malformed number, and not one that is
    // too large.
    let no_hex_digits = hex && digits.is_empty();
    // A leading zero before a digit is refused, as Python refuses `007`.
    let leading_zero =
        !hex && bytes[at] == b'0' && bytes.get(at + 1).is_some_and(u8::is_ascii_digit);
    if !underscores_fit || no_hex_digits || leading_zero {
        return refuse(end, Reason::MalformedNumber);
    }

    let digits = digits.replace('_', "");
    let kind = if float {
        // `str::parse` gives the nearest f64, and a value that is not finite
        // is too large.
        match digits.parse::<f64>() {
            Ok(v) if v.is_finite() => Tok::Float(v),
            Ok(_) => return refuse(end, Reason::FloatTooLarge),
            Err(_) => return refuse(end, Reason::MalformedNumber),
        }
    } else {
        match u64::from_str_radix(&digits, if hex { 16 } else { 10 }) {
            Ok(v) => Tok::Int(v),
            Err(_) => return refuse(end, Reason::IntTooLarge),
        }
    };
    Ok(Token { kind, at: at..end })
}

/// A text literal that starts at `at` and whose opening quote is at `open`.
/// `raw` keeps backslashes as they are.
fn text(text: &str, at: usize, open: usize, raw: bool) -> Result<Token, SyntaxError> {
    let refuse = |at: Range<usize>, reason| Err(SyntaxError { at, reason });
    let quote = char::from(text.as_bytes()[open]);
    // Three quotes in a row open a triple-quoted text, which is refused.
    if text[open..].chars().take(3).filter(|c| *c == quote).count() == 3 {
        return refuse(open..open + 3, Reason::TripleQuote);
    }

    let mut value = String::new();
    let mut chars = text[open + 1..].char_indices();
    while let Some((n, c)) = chars.next() {
        let n = open + 1 + n;
        if c == quote {
            let kind = Tok::Text(value);
            return Ok(Token {
                kind,
                at: at..n + 1,
            });
        }
        // At a line break the text is never closed, and the error points at
        // the literal from its start.
        if c == '\n' || c == '\r' {
            return refuse(at..n, Reason::UnclosedText);
        }
        if c != '\\' {
            value.push(c);
            continue;
        }
        let Some((_, escaped)) = chars.next().filter(|(_, c)| *c != '\n' && *c != '\r') else {
            break;
        };
        // A raw text keeps the backslash and the character after it, so
        // `r"\""` holds a backslash and a quote, as in Python.
        if raw {
            value.extend(['\\', escaped]);
            continue;
        }
        let end = n + 1 + escaped.len_utf8();
        let bad = |end: usize| {
            let found = text[n..end].to_owned();
            refuse(n..end, Reason::BadEscape { found })
        };
        value.push(match escaped {
            '\\' | '\'' | '"' => escaped,
            'n' => '\n',
            't' => '\t',
            'r' => '\r',
            'x' | 'u' => {
                // `\x` takes two hex digits and `\u` four, and they name a
                // character. A surrogate is a bad escape.
                let want = if escaped == 'x' { 2 } else { 4 };
                let hex = |c: &char| c.is_ascii_hexdigit();
                let digits: String = text[end..].chars().take(want).take_while(hex).collect();
                let code = u32::from_str_radix(&digits, 16).ok();
                let Some(c) = code
                    .filter(|_| digits.len() == want)
                    .and_then(char::from_u32)
                else {
                    return bad(end + digits.len());
                };
                chars.nth(want - 1);
                c
            }
            _ => return bad(end),
        });
    }
    refuse(at..text.len(), Reason::UnclosedText)
}
