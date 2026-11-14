//! The printer: a tree as rule text, with the fewest parentheses. A
//! canonical tree, which is a tree the parser can build, prints to a text
//! that parses back to the same tree. Any other tree prints as well as it
//! can.

use std::fmt;

use super::grammar::WORDS;
use crate::expr::{Expr, Segment, TypeName};

// How tightly a node binds. The levels are those of the grammar, from the
// conditional up to an atom.
const CONDITIONAL: u8 = 1;
const OR: u8 = 2;
const AND: u8 = 3;
const NOT: u8 = 4;
const COMPARISON: u8 = 5;
const ADDITIVE: u8 = 6;
const TERM: u8 = 7;
const UNARY: u8 = 8;
const ATOM: u8 = 9;
// A level that no node has, so the node is always put in parentheses.
const PARENTHESES: u8 = 10;

/// How tightly `e` binds, from 1 for the conditional to 9 for an atom.
fn level(e: &Expr) -> u8 {
    match e {
        Expr::If { .. } => CONDITIONAL,
        Expr::Or(_) => OR,
        Expr::And(_) => AND,
        Expr::Not(_) => NOT,
        Expr::Eq(..)
        | Expr::Ne(..)
        | Expr::Lt(..)
        | Expr::Le(..)
        | Expr::Gt(..)
        | Expr::Ge(..)
        | Expr::In(..)
        | Expr::NotIn(..)
        | Expr::IsNone(_)
        | Expr::IsNotNone(_) => COMPARISON,
        Expr::Add(..) | Expr::Sub(..) => ADDITIVE,
        Expr::Mul(..) | Expr::Div(..) | Expr::Mod(..) => TERM,
        Expr::Neg(_) => UNARY,
        _ => ATOM,
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write(self, CONDITIONAL, &mut Vec::new(), f)
    }
}

/// Writes `e`, in parentheses when it binds looser than `min`. `scope` holds
/// the element names bound at this point.
#[expect(clippy::too_many_lines, reason = "one arm per kind of node")]
fn write(e: &Expr, min: u8, scope: &mut Vec<String>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if level(e) < min {
        f.write_str("(")?;
        write(e, CONDITIONAL, scope, f)?;
        return f.write_str(")");
    }
    // A comparison writes both operands one level tighter, because
    // comparisons do not chain.
    let compared = (ADDITIVE, ADDITIVE);
    // `+`, `-`, `*`, `/` and `%` write the right operand one level tighter,
    // because they group to the left.
    let (added, multiplied) = ((ADDITIVE, TERM), (TERM, UNARY));
    match e {
        Expr::None => f.write_str("None"),
        Expr::Bool(true) => f.write_str("True"),
        Expr::Bool(false) => f.write_str("False"),
        Expr::Int(v) => write!(f, "{v}"),
        // `{:?}` gives the shortest text that parses back to the same value,
        // and it always has a `.` or an `e`, so it reads back as a float.
        Expr::Float(v) => write!(f, "{v:?}"),
        Expr::Text(s) => write_text(s, f),
        Expr::List(items) => {
            f.write_str("(")?;
            write_joined(items, ", ", CONDITIONAL, scope, f)?;
            // A list of one keeps its comma, because `(a)` reads back as
            // `a`.
            f.write_str(if items.len() == 1 { ",)" } else { ")" })
        }
        Expr::Field(path) => match path.first() {
            // The first key is written bare when it is a plain name that no
            // element in scope has.
            Some(Segment::Key(key)) if plain(key) && !scope.contains(key) => {
                f.write_str(key)?;
                write_steps(&path[1..], f)
            }
            // `record.key` under an element of the same name, and
            // `record["key"]` for a key that is no plain name.
            _ => {
                f.write_str("record")?;
                write_steps(path, f)
            }
        },
        Expr::Element { name, path } => {
            f.write_str(name)?;
            write_steps(path, f)
        }
        Expr::Not(x) => {
            f.write_str("not ")?;
            write(x, NOT, scope, f)
        }
        Expr::And(operands) => write_joined(operands, " and ", NOT, scope, f),
        Expr::Or(operands) => write_joined(operands, " or ", AND, scope, f),
        Expr::Eq(l, r) => write_infix(l, "==", r, compared, scope, f),
        Expr::Ne(l, r) => write_infix(l, "!=", r, compared, scope, f),
        Expr::Lt(l, r) => write_infix(l, "<", r, compared, scope, f),
        Expr::Le(l, r) => write_infix(l, "<=", r, compared, scope, f),
        Expr::Gt(l, r) => write_infix(l, ">", r, compared, scope, f),
        Expr::Ge(l, r) => write_infix(l, ">=", r, compared, scope, f),
        Expr::In(l, r) => write_infix(l, "in", r, compared, scope, f),
        Expr::NotIn(l, r) => write_infix(l, "not in", r, compared, scope, f),
        Expr::IsNone(x) => {
            write(x, ADDITIVE, scope, f)?;
            f.write_str(" is None")
        }
        Expr::IsNotNone(x) => {
            write(x, ADDITIVE, scope, f)?;
            f.write_str(" is not None")
        }
        Expr::Add(l, r) => write_infix(l, "+", r, added, scope, f),
        Expr::Sub(l, r) => write_infix(l, "-", r, added, scope, f),
        Expr::Mul(l, r) => write_infix(l, "*", r, multiplied, scope, f),
        Expr::Div(l, r) => write_infix(l, "/", r, multiplied, scope, f),
        Expr::Mod(l, r) => write_infix(l, "%", r, multiplied, scope, f),
        Expr::Neg(x) => {
            f.write_str("-")?;
            // A number under a `Neg` goes in parentheses, because `-5` reads
            // back as a negative literal.
            let number = matches!(**x, Expr::Int(_) | Expr::Float(_));
            write(x, if number { PARENTHESES } else { UNARY }, scope, f)
        }
        Expr::If {
            test,
            then,
            otherwise,
        } => {
            // The first branch and the test are written at the level of
            // `or`, and the branch after `else` at the level of the
            // conditional, because it groups to the right.
            write(then, OR, scope, f)?;
            f.write_str(" if ")?;
            write(test, OR, scope, f)?;
            f.write_str(" else ")?;
            write(otherwise, CONDITIONAL, scope, f)
        }
        Expr::Any { name, list, test } => write_quantifier("any", name, list, test, scope, f),
        Expr::All { name, list, test } => write_quantifier("all", name, list, test, scope, f),
        Expr::Len(x) => write_call("len", None, x, scope, f),
        Expr::IsNan(x) => write_call("math.isnan", None, x, scope, f),
        Expr::ReSearch { pattern, text } => write_call("re.search", Some(pattern), text, scope, f),
        Expr::ReMatch { pattern, text } => write_call("re.match", Some(pattern), text, scope, f),
        Expr::ReFullmatch { pattern, text } => {
            write_call("re.fullmatch", Some(pattern), text, scope, f)
        }
        Expr::IsInstance { value, types } => {
            f.write_str("isinstance(")?;
            write(value, CONDITIONAL, scope, f)?;
            let names: Vec<&str> = types.iter().map(|t| type_name(*t)).collect();
            // One type is written bare, and any other number as a list.
            match names.as_slice() {
                [one] => write!(f, ", {one})"),
                names => write!(f, ", ({}))", names.join(", ")),
            }
        }
        Expr::StartsWith(x, a) => write_method(x, "startswith", Some(a), scope, f),
        Expr::EndsWith(x, a) => write_method(x, "endswith", Some(a), scope, f),
        Expr::Upper(x) => write_method(x, "upper", None, scope, f),
        Expr::Lower(x) => write_method(x, "lower", None, scope, f),
        Expr::Strip(x) => write_method(x, "strip", None, scope, f),
        Expr::LStrip(x) => write_method(x, "lstrip", None, scope, f),
        Expr::RStrip(x) => write_method(x, "rstrip", None, scope, f),
        Expr::IsDigit(x) => write_method(x, "isdigit", None, scope, f),
        Expr::IsAlpha(x) => write_method(x, "isalpha", None, scope, f),
        Expr::IsAlnum(x) => write_method(x, "isalnum", None, scope, f),
        Expr::IsSpace(x) => write_method(x, "isspace", None, scope, f),
        Expr::IsUpper(x) => write_method(x, "isupper", None, scope, f),
        Expr::IsLower(x) => write_method(x, "islower", None, scope, f),
        Expr::IsAscii(x) => write_method(x, "isascii", None, scope, f),
    }
}

/// Writes `items` with `between` between them, each at `min` or tighter.
fn write_joined(
    items: &[Expr],
    between: &str,
    min: u8,
    scope: &mut Vec<String>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    for (n, item) in items.iter().enumerate() {
        if n > 0 {
            f.write_str(between)?;
        }
        write(item, min, scope, f)?;
    }
    Ok(())
}

/// Writes `l op r`, with the left operand at `mins.0` or tighter and the
/// right operand at `mins.1` or tighter.
fn write_infix(
    l: &Expr,
    op: &str,
    r: &Expr,
    mins: (u8, u8),
    scope: &mut Vec<String>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    write(l, mins.0, scope, f)?;
    write!(f, " {op} ")?;
    write(r, mins.1, scope, f)
}

/// Writes `function(test for name in list)`.
fn write_quantifier(
    function: &str,
    name: &str,
    list: &Expr,
    test: &Expr,
    scope: &mut Vec<String>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    write!(f, "{function}(")?;
    // The test is written with the name in scope and the list without it.
    scope.push(name.to_owned());
    let written = write(test, CONDITIONAL, scope, f);
    scope.pop();
    written?;
    write!(f, " for {name} in ")?;
    // The list is written at the level of `or`, which is where the grammar
    // reads it.
    write(list, OR, scope, f)?;
    f.write_str(")")
}

/// Writes `function(argument)`, or `function(pattern, argument)` for a
/// pattern function.
fn write_call(
    function: &str,
    pattern: Option<&str>,
    argument: &Expr,
    scope: &mut Vec<String>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    write!(f, "{function}(")?;
    if let Some(pattern) = pattern {
        // The raw form cannot hold a `"` or a line break, and a backslash at
        // its end would take the closing quote with it.
        let raw = !pattern.contains(['"', '\n', '\r']) && !pattern.ends_with('\\');
        if raw {
            write!(f, "r\"{pattern}\"")?;
        } else {
            write_text(pattern, f)?;
        }
        f.write_str(", ")?;
    }
    write(argument, CONDITIONAL, scope, f)?;
    f.write_str(")")
}

/// Writes `receiver.name()`, or `receiver.name(argument)`.
fn write_method(
    receiver: &Expr,
    name: &str,
    argument: Option<&Expr>,
    scope: &mut Vec<String>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    // A number goes in parentheses, because `5.upper()` reads as a float
    // followed by a name.
    let number = matches!(receiver, Expr::Int(_) | Expr::Float(_));
    write(receiver, if number { PARENTHESES } else { ATOM }, scope, f)?;
    write!(f, ".{name}(")?;
    if let Some(argument) = argument {
        write(argument, CONDITIONAL, scope, f)?;
    }
    f.write_str(")")
}

/// Writes the steps of a path after its start. A key that is a plain name is
/// written after a dot, and any other key in brackets.
fn write_steps(path: &[Segment], f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for step in path {
        match step {
            Segment::Key(key) if plain(key) => write!(f, ".{key}")?,
            Segment::Key(key) => {
                f.write_str("[")?;
                write_text(key, f)?;
                f.write_str("]")?;
            }
            Segment::Index(index) => write!(f, "[{index}]")?,
        }
    }
    Ok(())
}

/// Whether `key` can stand as a bare name: a name token that is no word of
/// the language.
fn plain(key: &str) -> bool {
    let mut chars = key.chars();
    let starts = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    starts && chars.all(|c| c.is_ascii_alphanumeric() || c == '_') && !WORDS.contains(&key)
}

/// Writes a text literal in double quotes, with the escapes the tokenizer
/// reads.
fn write_text(s: &str, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str("\"")?;
    for c in s.chars() {
        match c {
            '\\' => f.write_str("\\\\")?,
            '"' => f.write_str("\\\"")?,
            '\n' => f.write_str("\\n")?,
            '\t' => f.write_str("\\t")?,
            '\r' => f.write_str("\\r")?,
            // Every control character is below U+0100, so two hex digits
            // name it.
            c if c.is_control() => write!(f, "\\x{:02x}", u32::from(c))?,
            c => write!(f, "{c}")?,
        }
    }
    f.write_str("\"")
}

/// The name an author types for `t`.
fn type_name(t: TypeName) -> &'static str {
    match t {
        TypeName::Int => "int",
        TypeName::Float => "float",
        TypeName::Str => "str",
        TypeName::Bool => "bool",
        TypeName::List => "list",
    }
}
