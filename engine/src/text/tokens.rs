//! The tokenizer: it splits a rule's text into names, literals and
//! punctuation, each with its place. It does not fail. A text it cannot
//! split ends in a token that holds the error.

use std::ops::Range;

use super::error::{Reason, SyntaxError};
use crate::limits::MAX_TEXT_LENGTH;

/// What a token is.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Tok {
    // A word of the language, such as `and`, is a name here. The grammar
    // tells them apart.
    Name(String),
    // The magnitude without a sign, as a u64, so the grammar can fold a
    // leading minus into the smallest i64, whose magnitude no i64 holds.
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

// The operators and the punctuation of the rule language, whose
// Python-style syntax ADR-0041 sets. A longer one stands before its prefix,
// because the search in `next_token` takes the first entry that matches, so
// `<=` is not read as `<`.
const PUNCTS: [&str; 17] = [
    "==", "!=", "<=", ">=", "<", ">", "+", "-", "*", "/", "%", "(", ")", "[", "]", ",", ".",
];

// Operators of other languages and ours for each. The parser was designed
// on a list of faulty texts, each with a target for its error (ADR-0042).
// `FOREIGN` holds the foreign operators of those texts: `amount = 100`,
// `a > 1 && b < 2`, `a > 1 || b < 2`, `!active` and `status <> "paid"`.
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
    // The check counts characters and runs before any other work, so a text
    // over the cap costs nothing more. `cut` is the byte where the first
    // character past the cap starts.
    if let Some((cut, _)) = text.char_indices().nth(MAX_TEXT_LENGTH) {
        let kind = Tok::Bad(Reason::TooLong {
            max: MAX_TEXT_LENGTH,
        });
        // The bad token covers the characters beyond the cap.
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
            Err(SyntaxError { at: span, reason }) => {
                let kind = Tok::Bad(reason);
                tokens.push(Token { kind, at: span });
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
    // The function looks at the character at `at` and tries these cases in
    // order: a name or a text with a prefix, a number, a text, a comment, a
    // two-character operator of another language, an operator or punctuation
    // of the language, a one-character operator of another language, and any
    // other character. The first case that applies returns its token or its
    // error. The place of three cases matters: the number stands before the
    // punctuation, the two-character foreign operators before `PUNCTS`, and
    // the one-character foreign operators after it. The note at each of
    // those cases says why.
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

    // A name is ASCII. A word that starts with another letter falls through
    // to the last case, as an unexpected character.
    if c.is_ascii_alphabetic() || c == '_' {
        // `end` is the byte after the name.
        let end = at + rest.find(|c| !is_name_char(c)).unwrap_or(rest.len());
        let word = &text[at..end];
        let quoted = text[end..].starts_with(['"', '\'']);
        // A word of at most two letters, each of `r`, `b`, `u` or `f` in
        // either case, counts as a prefix when a quote follows it directly.
        // Python's prefixes, such as `rb` and `f`, have at most two letters.
        // The test also admits combinations that Python does not have, such
        // as `uu`, which this case then refuses as a text prefix.
        let prefixed = quoted && word.len() <= 2 && word.chars().all(|c| "rbufRBUF".contains(c));
        // Any other word before a quote is a name, so `x in"a"` reads as it
        // does in Python.
        if !prefixed {
            let kind = Tok::Name(word.to_owned());
            return Ok(Token { kind, at: at..end });
        }
        // The literal is read with `raw` set whatever the prefix, so a bad
        // escape such as the `\q` of `b"\q"` cannot fail it before the
        // prefix is refused. A literal that cannot be read even raw, such as
        // one that is never closed, returns its own error through the `?`.
        let literal = quoted_text(text, at, end, true)?;
        // `r` and `R` start a raw text, and every other prefix is refused.
        // `r"\d"` is one token, from the `r` to the closing quote.
        if word.eq_ignore_ascii_case("r") {
            return Ok(literal);
        }
        let prefix = word.to_owned();
        // The refusal covers the whole literal: bytes 0 to 6 of
        // `f"{a}" == "x"`.
        return refuse(literal.at.len(), Reason::TextPrefix { prefix });
    }
    // This case stands before the punctuation case, because `PUNCTS` holds
    // `.`. A `.` followed by a digit starts a float here, as in `.5`, and
    // any other `.` reaches the punctuation case, as in `item.price`.
    let digit_next = rest[c.len_utf8()..].starts_with(|c: char| c.is_ascii_digit());
    if c.is_ascii_digit() || (c == '.' && digit_next) {
        return number(text, at);
    }
    if c == '"' || c == '\'' {
        // A text in either quote is read with its escapes decoded, which is
        // what the `false` argument means.
        return quoted_text(text, at, at, false);
    }
    // A comment is refused, because the tree cannot carry it and it would
    // disappear when the database keeps the tree alone (ADR-0041).
    if c == '#' {
        // The error covers the comment up to the end of its line: bytes 13
        // to 23 of `amount > 100 # big ones`.
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
    // The two-character foreign operators are tried before `PUNCTS`, which
    // would read the `<` of the `<>` in `status <> "paid"`.
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
    // The one-character foreign operators are tried after `PUNCTS`, which
    // reads `==` and `!=` first, so only a lone `=` or `!` is refused. In
    // `a === b` the `==` is read first, and the third `=` is refused at
    // byte 4.
    if let Some(refusal) = foreign(false) {
        return refusal;
    }
    // Any character that no case above took is refused as an unexpected
    // character, as the `$` of `amount > $100`.
    refuse(c.len_utf8(), Reason::UnexpectedCharacter { found: c })
}

/// A number that starts at `at`: a decimal or hexadecimal integer, or a
/// float. It fails with `MalformedNumber`, `IntTooLarge` or `FloatTooLarge`,
/// in an error whose range starts at `at`.
fn number(text: &str, at: usize) -> Result<Token, SyntaxError> {
    // The function first finds where the number ends, with a scan that
    // admits underscores anywhere. Then it refuses what the scan let through,
    // and then it converts the digits. Every error starts at `at`. The error
    // for a trailing name character covers the whole word, and every other
    // error covers the number that the scan read.
    //
    // 1. We scan the extent of the number. It is a `0x` or `0X` prefix and
    //    its hexadecimal digits, or decimal digits with an optional fraction
    //    and an optional exponent, which needs a digit. `float` records
    //    whether a fraction or an exponent was read.
    // 2. We refuse the number as malformed when a name character follows it.
    //    The error covers the whole word.
    // 3. We check the digits. Every underscore stands between two digits, a
    //    `0x` has digits after it, and a decimal number has no leading zero
    //    that a digit follows.
    // 4. We drop the underscores and convert the digits to an `f64` or a
    //    `u64`. A float that is not finite and an integer above `u64::MAX`
    //    are refused.

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

    // 1. scan the extent
    // `0x10` and `0X10` are hexadecimal, and their digits start after the
    // `x`.
    let hex = bytes[at] == b'0' && matches!(bytes.get(at + 1), Some(b'x' | b'X'));
    // `end` is the byte after the number read so far.
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
            let sign_len = usize::from(matches!(bytes.get(end + 1), Some(b'+' | b'-')));
            if bytes
                .get(end + 1 + sign_len)
                .is_some_and(u8::is_ascii_digit)
            {
                float = true;
                end = run(end + 1 + sign_len, false);
            }
        }
    }

    // 2. refuse a trailing name character
    // `1abc` is then one error, and not a number and a name.
    let word_end = end
        + text[end..]
            .find(|c| !is_name_char(c))
            .unwrap_or(text.len() - end);
    if word_end > end {
        return refuse(word_end, Reason::MalformedNumber);
    }
    // 3. check the digits
    // The digits without the `0x` of a hexadecimal number.
    let digits = &text[if hex { at + 2 } else { at }..end];
    // The marks `.`, `e`, `E`, `+` and `-` split a float into parts, which
    // are checked one by one. A hexadecimal number is one part, because `e`
    // is one of its digits, as in `0x1e_5`.
    let marks: &[char] = if hex { &[] } else { &['.', 'e', 'E', '+', '-'] };
    // An underscore stands between two digits. `1_000` passes, and `1_` and
    // `1__0` fail.
    let underscores_fit = digits
        .split(marks)
        .all(|part| !part.starts_with('_') && !part.ends_with('_') && !part.contains("__"));
    // Without this check, `u64::from_str_radix` fails on the empty digits of
    // `0x`, and the match below reports `IntTooLarge` where
    // `MalformedNumber` is right.
    let no_hex_digits = hex && digits.is_empty();
    // A 0 that a digit follows, with or without underscores between them, is
    // refused, as Python refuses `007` and `0_7`. `0`, `0.5` and `0x10` pass.
    // The check also refuses `00` and `01.5`, which Python accepts.
    let leading_zero = !hex
        && bytes[at] == b'0'
        && bytes[at + 1..end]
            .iter()
            .find(|b| **b != b'_')
            .is_some_and(u8::is_ascii_digit);
    if !underscores_fit || no_hex_digits || leading_zero {
        return refuse(end, Reason::MalformedNumber);
    }

    // 4. convert the digits
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

/// Reads a text literal whose opening quote is at `open`. `at` is where its
/// prefix starts, and it equals `open` for a text without a prefix. `raw`
/// keeps backslashes as they are. Returns a `Text` token from `at` to the
/// closing quote.
///
/// Fails with `TripleQuote` for three quotes, with `UnclosedText` for a line
/// break or the end of the text before the closing quote, and with
/// `BadEscape` for an unknown or short escape when `raw` is false.
fn quoted_text(text: &str, at: usize, open: usize, raw: bool) -> Result<Token, SyntaxError> {
    let refuse = |span: Range<usize>, reason| Err(SyntaxError { at: span, reason });
    // The quote that opens the text also closes it, so `"it's"` holds an
    // apostrophe.
    let quote = char::from(text.as_bytes()[open]);
    // Three quotes in a row open a triple-quoted text, which is refused:
    // bytes 0 to 3 of `"""abc""" == x`. The error covers the three quotes
    // alone, also after a prefix. The empty text `""` has two.
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
        // nothing, and the text is never closed. The error then runs to the
        // end of the whole text, also past the line break. After a bare line
        // break the error ends at the break.
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
        // The error covers the escape from its backslash up to `stop`: bytes
        // 10 to 12, the `\q`, of `name == 'a\qb'`.
        let bad = |stop: usize| {
            let found = text[n..stop].to_owned();
            refuse(n..stop, Reason::BadEscape { found })
        };
        // These eight of Python's escapes are a first set. Any other escape
        // is a `BadEscape`.
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
    // Two paths reach this line: the text ends before a closing quote, or
    // the loop breaks after a backslash at the end of the text or before a
    // line break. The error covers the literal to the end of the text: bytes
    // 8 to 12 of `name == "abc`.
    refuse(at..text.len(), Reason::UnclosedText)
}
