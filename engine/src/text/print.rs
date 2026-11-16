//! The printer writes a tree as rule text, and it puts a node in parentheses
//! where the grammar would otherwise read the text as another tree. A number
//! under a minus and a number before a method always get parentheses,
//! whatever their sign or kind, as in `-(-5)` and `(0.5).upper()`.
//!
//! A canonical tree, which is a tree the parser can build, prints to a text
//! that parses back to the same tree. `Display` also writes a tree that the
//! parser cannot build, such as an `and` directly under an `and`, or a path
//! that is empty or starts with an index. Such a path prints as `record` or
//! `record[0]`, which the parser refuses as `BareRecord`. The parser may
//! refuse the text of such a tree or read it back as another tree.

use std::fmt;

use super::grammar::WORDS;
use crate::expr::{Expr, Segment, TypeName};

// These constants say how tightly each kind of node binds, in the order of
// the grammar's levels from the conditional up to an atom. The grammar's
// postfix forms, such as `a.upper()` and `items[0]`, share ATOM with the
// atoms. A node is written bare where a level at most its own is wanted, and
// in parentheses elsewhere.
// `a if b else c`.
const CONDITIONAL: u8 = 1;
// `a or b`.
const OR: u8 = 2;
// `a and b`.
const AND: u8 = 3;
// `not a`.
const NOT: u8 = 4;
// `a < b`, `a not in b` and `a is None`.
const COMPARISON: u8 = 5;
// `a + b` and `a - b`.
const ADDITIVE: u8 = 6;
// `a * b`, `a / b` and `a % b`.
const TERM: u8 = 7;
// `-a`.
const UNARY: u8 = 8;
// `a`, `5`, `"paid"`, `(a, b)`, `len(a)` and `a.upper()`.
const ATOM: u8 = 9;
// PARENTHESES lies above the level of every node, so `write_expr` always
// puts a node in parentheses when asked for it.
const PARENTHESES: u8 = 10;

/// How tightly `e` binds, from CONDITIONAL, the loosest, up to ATOM.
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
        write_expr(self, CONDITIONAL, &mut Vec::new(), f)
    }
}

/// Writes `e`, in parentheses when it binds looser than `min`. `scope` holds
/// the element names bound at this point.
#[expect(clippy::too_many_lines, reason = "one arm per kind of node")]
fn write_expr(
    e: &Expr,
    min: u8,
    scope: &mut Vec<String>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    // `e` binds looser than its place allows. In parentheses any level
    // fits again. `And([Or([a, b]), c])` prints as `(a or b) and c`,
    // because `a or b and c` reads back as `a or (b and c)`.
    if level(e) < min {
        f.write_str("(")?;
        write_expr(e, CONDITIONAL, scope, f)?;
        return f.write_str(")");
    }
    // A comparison writes both operands one level tighter, because
    // comparisons do not chain. `Lt(Lt(a, b), c)` prints as `(a < b) < c`,
    // because the parser refuses `a < b < c`.
    let compared = (ADDITIVE, ADDITIVE);
    // `+`, `-`, `*`, `/` and `%` write the right operand one level tighter,
    // because they group to the left. `Sub(a, Sub(b, c))` prints as
    // `a - (b - c)`, because `a - b - c` reads back as `(a - b) - c`.
    let (added, multiplied) = ((ADDITIVE, TERM), (TERM, UNARY));
    match e {
        Expr::None => f.write_str("None"),
        Expr::Bool(true) => f.write_str("True"),
        Expr::Bool(false) => f.write_str("False"),
        // A negative value prints as `-5`, and the grammar's rule `unary`
        // reads that minus and the number as one negative literal. This also
        // holds for the smallest integer, `-9223372036854775808`.
        Expr::Int(v) => write!(f, "{v}"),
        // `{:?}` gives the shortest text that parses back to the same value.
        // A float in a tree is finite, and for a finite value that text
        // always has a `.` or an `e`, so it reads back as a float.
        Expr::Float(v) => write!(f, "{v:?}"),
        Expr::Text(s) => write_text(s, f),
        Expr::List(items) => {
            f.write_str("(")?;
            write_joined(items, ", ", CONDITIONAL, scope, f)?;
            // A list of one keeps its comma, because `(a)` reads back as
            // `a`. The empty list is `()`. A list written `[a, b]` reads
            // back to the same tree and prints as `(a, b)`.
            f.write_str(if items.len() == 1 { ",)" } else { ")" })
        }
        Expr::Field(path) => match path.first() {
            // The first key is written bare when it is a plain name that no
            // element in scope has: `amount`, and `items[0].price`.
            Some(Segment::Key(key)) if plain(key) && !scope.contains(key) => {
                f.write_str(key)?;
                write_steps(&path[1..], f)
            }
            // A first key that an element in scope also has is written as
            // `record.key`, because a bare `i` under
            // `any(i > record.i for i in items)` reads back as the element.
            // A first key that is no plain name needs `record` before its
            // bracket, as in `record["unit price"]`.
            _ => {
                f.write_str("record")?;
                write_steps(path, f)
            }
        },
        // In a canonical tree a quantifier around the node binds the name,
        // so the bare name reads back as this element: `i.price` in
        // `any(i.price > 100 for i in items)`.
        Expr::Element { name, path } => {
            f.write_str(name)?;
            write_steps(path, f)
        }
        // `not not a` and `not a == 1` need no parentheses, and
        // `not (a and b)` does, because `not a and b` reads back as
        // `(not a) and b`.
        Expr::Not(x) => {
            f.write_str("not ")?;
            write_expr(x, NOT, scope, f)
        }
        // An operand of `and` is at the level of `not` or tighter, so an
        // `or` among them gets parentheses, as in `a and (b or c)`. An `and`
        // under an `and` is no canonical tree, because the parser joins
        // `(a and b) and c` into one list.
        Expr::And(operands) => write_joined(operands, " and ", NOT, scope, f),
        // An operand of `or` is at the level of `and` or tighter, so a
        // conditional among them gets parentheses.
        Expr::Or(operands) => write_joined(operands, " or ", AND, scope, f),
        Expr::Eq(l, r) => write_infix(l, "==", r, compared, scope, f),
        Expr::Ne(l, r) => write_infix(l, "!=", r, compared, scope, f),
        Expr::Lt(l, r) => write_infix(l, "<", r, compared, scope, f),
        Expr::Le(l, r) => write_infix(l, "<=", r, compared, scope, f),
        Expr::Gt(l, r) => write_infix(l, ">", r, compared, scope, f),
        Expr::Ge(l, r) => write_infix(l, ">=", r, compared, scope, f),
        Expr::In(l, r) => write_infix(l, "in", r, compared, scope, f),
        Expr::NotIn(l, r) => write_infix(l, "not in", r, compared, scope, f),
        // The operand is written one level tighter than a comparison, as
        // the operands of `<` are. `IsNone(Lt(a, b))` prints as
        // `(a < b) is None`, because the parser refuses `a < b is None`.
        Expr::IsNone(x) => {
            write_expr(x, ADDITIVE, scope, f)?;
            f.write_str(" is None")
        }
        Expr::IsNotNone(x) => {
            write_expr(x, ADDITIVE, scope, f)?;
            f.write_str(" is not None")
        }
        Expr::Add(l, r) => write_infix(l, "+", r, added, scope, f),
        Expr::Sub(l, r) => write_infix(l, "-", r, added, scope, f),
        Expr::Mul(l, r) => write_infix(l, "*", r, multiplied, scope, f),
        Expr::Div(l, r) => write_infix(l, "/", r, multiplied, scope, f),
        Expr::Mod(l, r) => write_infix(l, "%", r, multiplied, scope, f),
        Expr::Neg(x) => {
            f.write_str("-")?;
            // Every number under a `Neg` goes in parentheses, whatever its
            // sign, so `Neg(Int(5))` prints as `-(5)` and `Neg(Int(-5))` as
            // `-(-5)`. For a number that is not negative the reason is that
            // `-5` reads back as a negative literal. A negative number would
            // read back the same without them and gets them all the same.
            let number = matches!(**x, Expr::Int(_) | Expr::Float(_));
            // Any other operand is written at the level of a minus, as in
            // `-a`, `--a` and `-(a + b)`.
            write_expr(x, if number { PARENTHESES } else { UNARY }, scope, f)
        }
        Expr::If {
            test,
            then,
            otherwise,
        } => {
            // The grammar reads the first branch and the test with
            // `or_expr`, so a conditional in either place needs parentheses,
            // as in `(a if b else c) if d else e`. The branch after `else`
            // may be a conditional itself, because the conditional groups to
            // the right.
            write_expr(then, OR, scope, f)?;
            f.write_str(" if ")?;
            write_expr(test, OR, scope, f)?;
            f.write_str(" else ")?;
            write_expr(otherwise, CONDITIONAL, scope, f)
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
            write_expr(value, CONDITIONAL, scope, f)?;
            let names: Vec<&str> = types.iter().map(|t| type_name(*t)).collect();
            // One type is written bare, and any other number as a list.
            // `isinstance(x, (int,))` reads back to the same tree as
            // `isinstance(x, int)`.
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
        write_expr(item, min, scope, f)?;
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
    write_expr(l, mins.0, scope, f)?;
    write!(f, " {op} ")?;
    write_expr(r, mins.1, scope, f)
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
    // The name is in scope only while the test is written, because the
    // parser resolves the list outside the name and the test inside it.
    // `scope` decides whether a field is written as `record.i`.
    scope.push(name.to_owned());
    let written = write_expr(test, CONDITIONAL, scope, f);
    // The name is popped before `?` can return the error of the test, so
    // `scope` is the same on every way out.
    scope.pop();
    written?;
    write!(f, " for {name} in ")?;
    // The list is written at the level of `or`, which is where the grammar
    // reads it. A conditional as the list prints in parentheses, because
    // the grammar ends the list at the `if`.
    write_expr(list, OR, scope, f)?;
    f.write_str(")")
}

/// Writes `function(argument)`, or `function(pattern, argument)` for
/// `re.search`, `re.match` and `re.fullmatch`.
fn write_call(
    function: &str,
    pattern: Option<&str>,
    argument: &Expr,
    scope: &mut Vec<String>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    write!(f, "{function}(")?;
    if let Some(pattern) = pattern {
        // A raw text ends at the first `"` that no backslash escapes, it
        // cannot hold a line break, and a backslash at its end would take
        // the closing quote with it. The check is stricter than the
        // tokenizer: it rejects any `"`, also one after a backslash, which a
        // raw text could hold.
        let raw = !pattern.contains(['"', '\n', '\r']) && !pattern.ends_with('\\');
        // The pattern is written raw where it can be, as `r"^\d{5}$"`,
        // because the escaped form doubles every backslash. Both forms read
        // back to the same pattern. The pattern `a"b` cannot be raw and is
        // written `"a\"b"`.
        if raw {
            write!(f, "r\"{pattern}\"")?;
        } else {
            write_text(pattern, f)?;
        }
        f.write_str(", ")?;
    }
    write_expr(argument, CONDITIONAL, scope, f)?;
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
    // A number goes in parentheses. The tokenizer reads `5.upper` as a
    // malformed float. In `-0.5.upper()` the grammar reads the minus and the
    // number as one literal and reads no step after it, so the text is
    // refused at the second `.`.
    let number = matches!(receiver, Expr::Int(_) | Expr::Float(_));
    // Any other receiver is an atom or in parentheses. `Upper(Add(a, b))`
    // prints as `(a + b).upper()`, because `a + b.upper()` reads back as
    // `a + (b.upper())`.
    write_expr(receiver, if number { PARENTHESES } else { ATOM }, scope, f)?;
    write!(f, ".{name}(")?;
    if let Some(argument) = argument {
        write_expr(argument, CONDITIONAL, scope, f)?;
    }
    f.write_str(")")
}

/// Writes each segment of `path` as a step after a name or after `record`: a
/// plain key after a dot, any other key in brackets and an index in brackets,
/// as in `.lines`, `["unit price"]` and `[2]`.
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
    // The first character and the rest follow the tokenizer's rule for a
    // name, so a plain key reads back as one name token.
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
            // A backslash would start an escape and a `"` would end the
            // text, so both are escaped.
            '\\' => f.write_str("\\\\")?,
            '"' => f.write_str("\\\"")?,
            // The tokenizer refuses a line break inside a text, so `\n` and
            // `\r` are always escaped.
            '\n' => f.write_str("\\n")?,
            // A tab is a control character and gets the short escape `\t`,
            // which the tokenizer reads. Without this arm the arm for control
            // characters below would write it as `\x09`.
            '\t' => f.write_str("\\t")?,
            '\r' => f.write_str("\\r")?,
            // Any other control character is written as an escape, so that
            // a printed rule shows it. Every control character is below
            // U+0100, so two hex digits name it.
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
