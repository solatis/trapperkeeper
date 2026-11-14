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
    // The characters between the quotes of a text literal. An escape is
    // replaced by its character, except in a raw text.
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
pub(super) fn lex(text: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    // The check counts characters and runs first, so a text over the cap
    // costs no further work. The bad token covers the characters beyond it.
    // `cut` is the byte where the first character past the cap starts.
    if let Some((cut, _)) = text.char_indices().nth(MAX_TEXT_LENGTH) {
        let kind = Tok::Bad(Reason::TooLong {
            max: MAX_TEXT_LENGTH,
        });
        let at = cut..text.len();
        tokens.push(Token { kind, at });
        return tokens;
    }
    // `at` is the byte where the next token or space starts. It always
    // falls between two characters, because it moves by whole characters
    // and whole tokens.
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
            // The error becomes the last token, and the rest of the text is
            // not read. In `amount > 100;` the list is `amount`, `>`, `100`
            // and the bad `;`.
            Err(SyntaxError { at, reason }) => {
                let kind = Tok::Bad(reason);
                tokens.push(Token { kind, at });
                return tokens;
            }
        }
    }
    // `End` has the empty range at the end of the text, so an error about a
    // missing token points there, as in `amount >`.
    let at = text.len()..text.len();
    tokens.push(Token { kind: Tok::End, at });
    tokens
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// The token that starts at byte `at`, or why none does.
fn next_token(text: &str, at: usize) -> Result<Token, SyntaxError> {
    // 1. a name, or a text with a prefix such as `r"\d"`
    // 2. a number
    // 3. a text
    // 4. a comment
    // 5. a two-character operator of another language
    // 6. an operator or punctuation of the language
    // 7. a one-character operator of another language
    // 8. any other character
    let rest = &text[at..];
    // An error from `at` that covers `len` bytes.
    let refuse = |len: usize, reason| {
        Err(SyntaxError {
            at: at..at + len,
            reason,
        })
    };
    let Some(c) = rest.chars().next() else {
        unreachable!("`lex` calls this at a character")
    };

    // 1. `amount` in `amount = 100`. Names are ASCII, and any other letter
    //    is an unexpected character at step 8.
    if c.is_ascii_alphabetic() || c == '_' {
        // `end` is the byte after the name.
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
        // literal is read raw either way, so the refusal covers all of it:
        // bytes 0 to 6 of `f"{a}" == "x"`. A literal that cannot be read,
        // such as one that is never closed, gets its own error instead.
        let literal = self::text(text, at, end, true)?;
        // `r"\d"` is one token, from the `r` to the closing quote.
        if word.eq_ignore_ascii_case("r") {
            return Ok(literal);
        }
        let prefix = word.to_owned();
        return refuse(literal.at.len(), Reason::TextPrefix { prefix });
    }
    // 2. `0x10`, `1_000` and `.5`. A `.` followed by a digit starts a float,
    //    and any other `.` is punctuation at step 6, as in `item.price`.
    let digit_next = rest[c.len_utf8()..].starts_with(|c: char| c.is_ascii_digit());
    if c.is_ascii_digit() || (c == '.' && digit_next) {
        return number(text, at);
    }
    // 3. `"paid"` and `'paid'`, read with their escapes.
    if c == '"' || c == '\'' {
        return self::text(text, at, at, false);
    }
    // 4. The error covers the comment to the end of its line: bytes 13 to
    //    23 of `amount > 100 # big ones`.
    if c == '#' {
        let len = rest.find('\n').unwrap_or(rest.len());
        return refuse(len, Reason::Comment);
    }
    // The first operator of `FOREIGN` that the rest starts with, among the
    // two-character ones when `long` is set and the one-character ones
    // otherwise, as an error that names ours.
    let foreign = |long: bool| {
        let fits =
            |(found, _): &&(&str, &str)| rest.starts_with(found) && (found.len() == 2) == long;
        FOREIGN.iter().find(fits).map(|&(found, write)| {
            let reason = Reason::ForeignOperator { found, write };
            refuse(found.len(), reason)
        })
    };
    // 5. `<>` in `status <> "paid"`, before `PUNCTS` would read its `<`.
    if let Some(refusal) = foreign(true) {
        return refusal;
    }
    // 6. `==`, `>`, `(` and the others, the longest first.
    if let Some(p) = PUNCTS.iter().find(|p| rest.starts_with(**p)) {
        let at = at..at + p.len();
        return Ok(Token {
            kind: Tok::Punct(p),
            at,
        });
    }
    // 7. The `=` of `amount = 100`, after `PUNCTS` has had `==`. In
    //    `a === b` the `==` is read first, and the third `=` is refused at
    //    byte 4.
    if let Some(refusal) = foreign(false) {
        return refusal;
    }
    // 8. `;` in `amount > 100;` and `$` in `amount > $100`.
    refuse(c.len_utf8(), Reason::UnexpectedCharacter { found: c })
}

/// A number that starts at `at`: a decimal or hexadecimal integer, or a
/// float.
fn number(text: &str, at: usize) -> Result<Token, SyntaxError> {
    // Every character of a number is ASCII, so the scan runs over bytes.
    let bytes = text.as_bytes();
    // An error from `at` up to `end`.
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

    // `0x10` and `0X10` are hexadecimal, and their digits start after the
    // `x`. `end` is the byte after the number read so far.
    let hex = bytes[at] == b'0' && matches!(bytes.get(at + 1), Some(b'x' | b'X'));
    let mut end = run(if hex { at + 2 } else { at }, hex);
    let mut float = false;
    // Only a decimal number has a fraction or an exponent.
    if !hex {
        // `3.5`, and `.5`, whose first run is empty.
        if bytes.get(end) == Some(&b'.') {
            float = true;
            end = run(end + 1, false);
        }
        // `1e999` and `2E-3`. The `e`, its sign if any and a digit make an
        // exponent. Without a digit the `e` is left, and the check below
        // refuses `1e` as malformed.
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
    // The digits without the `0x` of a hexadecimal number.
    let digits = &text[if hex { at + 2 } else { at }..end];
    // An underscore stands between two digits. `1_000` passes, and `1_`
    // and `1__0` fail. The parts between the marks of a float are checked
    // one by one.
    let underscores_fit = digits
        .split(['.', 'e', 'E', '+', '-'])
        .all(|part| !part.starts_with('_') && !part.ends_with('_') && !part.contains("__"));
    // `0x` with no digit after it is a malformed number, and not one that is
    // too large.
    let no_hex_digits = hex && digits.is_empty();
    // A leading zero before a digit is refused, as Python refuses `007`.
    // `0`, `0.5` and `0x10` pass.
    let leading_zero =
        !hex && bytes[at] == b'0' && bytes.get(at + 1).is_some_and(u8::is_ascii_digit);
    if !underscores_fit || no_hex_digits || leading_zero {
        return refuse(end, Reason::MalformedNumber);
    }

    let digits = digits.replace('_', "");
    let kind = if float {
        // `str::parse` gives the nearest f64, and a value that is not finite
        // is too large, as `1e999` is.
        match digits.parse::<f64>() {
            Ok(v) if v.is_finite() => Tok::Float(v),
            Ok(_) => return refuse(end, Reason::FloatTooLarge),
            // No text reaches this arm. The scan admits digits, one `.` and
            // an exponent with a digit, and `f64` parses every such text.
            Err(_) => return refuse(end, Reason::MalformedNumber),
        }
    } else {
        // The digits are not empty here, so the parse fails only for a value
        // above `u64::MAX`, as `99999999999999999999` is.
        match u64::from_str_radix(&digits, if hex { 16 } else { 10 }) {
            Ok(v) => Tok::Int(v),
            Err(_) => return refuse(end, Reason::IntTooLarge),
        }
    };
    Ok(Token { kind, at: at..end })
}

/// A text literal that starts at `at` and whose opening quote is at `open`.
/// `raw` keeps backslashes as they are.
// `at` and `open` differ by the prefix: for `r"\d"` they are the bytes of
// the `r` and of the quote, and for `"paid"` both are the quote.
fn text(text: &str, at: usize, open: usize, raw: bool) -> Result<Token, SyntaxError> {
    let refuse = |at: Range<usize>, reason| Err(SyntaxError { at, reason });
    // The quote that opens the text also closes it, so `"it's"` holds an
    // apostrophe.
    let quote = char::from(text.as_bytes()[open]);
    // Three quotes in a row open a triple-quoted text, which is refused:
    // bytes 0 to 3 of `"""abc""" == x`. The empty text `""` has two.
    if text[open..].chars().take(3).filter(|c| *c == quote).count() == 3 {
        return refuse(open..open + 3, Reason::TripleQuote);
    }

    let mut value = String::new();
    // `chars` runs over the text after the opening quote, and an escape
    // takes further characters from it.
    let mut chars = text[open + 1..].char_indices();
    while let Some((n, c)) = chars.next() {
        // `n` becomes the byte of `c` in the whole text.
        let n = open + 1 + n;
        // The token covers the prefix, both quotes and everything between.
        if c == quote {
            let kind = Tok::Text(value);
            return Ok(Token {
                kind,
                at: at..n + 1,
            });
        }
        // At a line break the text is never closed, and the error points at
        // the literal from its start up to the line break.
        if c == '\n' || c == '\r' {
            return refuse(at..n, Reason::UnclosedText);
        }
        if c != '\\' {
            value.push(c);
            continue;
        }
        // A backslash at the end of the text or before a line break escapes
        // nothing, and the text is never closed. The error then covers the
        // rest of the text.
        let Some((_, escaped)) = chars.next().filter(|(_, c)| *c != '\n' && *c != '\r') else {
            break;
        };
        // A raw text keeps the backslash and the character after it, so
        // `r"\""` holds a backslash and a quote, as in Python.
        if raw {
            value.extend(['\\', escaped]);
            continue;
        }
        // `end` is the byte after the escaped character, where the digits
        // of `\x` and `\u` start.
        let end = n + 1 + escaped.len_utf8();
        // The error covers the escape from its backslash up to `end`: bytes
        // 10 to 12, the `\q`, of `name == 'a\qb'`.
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
                // Too few digits is a bad escape that covers the digits
                // there are: the `\u12` of `name == "\u12"`.
                let Some(c) = code
                    .filter(|_| digits.len() == want)
                    .and_then(char::from_u32)
                else {
                    return bad(end + digits.len());
                };
                // The digits were read from `text`, so `chars` skips them.
                chars.nth(want - 1);
                c
            }
            _ => return bad(end),
        });
    }
    // The text ended before the closing quote: bytes 8 to 12 of
    // `name == "abc`.
    refuse(at..text.len(), Reason::UnclosedText)
}
