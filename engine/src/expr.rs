//! The expression tree of the rule language and its JSON form. In JSON one
//! key names the kind of a node, and a node refuses a key it does not know.
//! The kinds of expression and the closed list of functions and methods are
//! ADR-0041, and the form of the tree is ADR-0033.

use serde::{Deserialize, Serialize};

/// One step of a path: a key into an object, or an index into a list. In
/// JSON a key is a text and an index is a number.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Segment {
    Key(String),
    Index(u32),
}

/// A type that `isinstance` tests for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TypeName {
    Int,
    Float,
    Str,
    Bool,
    List,
}

/// One node of the tree. A rule's expression is the node at the top. The
/// tree keeps the operators as the author wrote them, so `!=`, `not in` and
/// `is not None` have kinds of their own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Expr {
    // Literals.
    None,
    Bool(bool),
    Int(i64),
    // JSON has no form for a NaN or an infinity, so a float in a tree must
    // be finite.
    Float(f64),
    Text(String),
    List(Vec<Expr>),

    // Fields.
    // The path starts at the record.
    Field(Vec<Segment>),
    // The path starts at the element that an enclosing `Any` or `All` names.
    // An empty path is the element itself.
    Element {
        name: String,
        path: Vec<Segment>,
    },

    // Logic.
    Not(Box<Expr>),
    // `And` and `Or` hold the operands of one chain in written order, two
    // or more. An operand of the same kind belongs in the same list, so one
    // chain is one node however the author grouped it.
    And(Vec<Expr>),
    Or(Vec<Expr>),

    // Comparisons. Each holds its left and its right operand.
    Eq(Box<Expr>, Box<Expr>),
    Ne(Box<Expr>, Box<Expr>),
    Lt(Box<Expr>, Box<Expr>),
    Le(Box<Expr>, Box<Expr>),
    Gt(Box<Expr>, Box<Expr>),
    Ge(Box<Expr>, Box<Expr>),
    In(Box<Expr>, Box<Expr>),
    NotIn(Box<Expr>, Box<Expr>),
    IsNone(Box<Expr>),
    IsNotNone(Box<Expr>),

    // Arithmetic.
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Mod(Box<Expr>, Box<Expr>),
    Neg(Box<Expr>),

    // The conditional, `then if test else otherwise`.
    If {
        test: Box<Expr>,
        then: Box<Expr>,
        // `else` is a keyword of Rust, so the field has another name and
        // JSON keeps `else`.
        #[serde(rename = "else")]
        otherwise: Box<Expr>,
    },

    // The quantifiers, `any(test for name in list)`.
    Any {
        name: String,
        list: Box<Expr>,
        test: Box<Expr>,
    },
    All {
        name: String,
        list: Box<Expr>,
        test: Box<Expr>,
    },

    // The functions. Each key is the name an author types, with `_` for the
    // dot.
    Len(Box<Expr>),
    #[serde(rename = "isinstance")]
    IsInstance {
        value: Box<Expr>,
        types: Vec<TypeName>,
    },
    // The pattern of the three pattern functions is a text and no
    // expression, so it can be checked when the rule is created.
    ReSearch {
        pattern: String,
        text: Box<Expr>,
    },
    ReMatch {
        pattern: String,
        text: Box<Expr>,
    },
    ReFullmatch {
        pattern: String,
        text: Box<Expr>,
    },
    #[serde(rename = "math_isnan")]
    IsNan(Box<Expr>),

    // The methods. The first operand is the text the method is called on.
    #[serde(rename = "startswith")]
    StartsWith(Box<Expr>, Box<Expr>),
    #[serde(rename = "endswith")]
    EndsWith(Box<Expr>, Box<Expr>),
    Upper(Box<Expr>),
    Lower(Box<Expr>),
    Strip(Box<Expr>),
    #[serde(rename = "lstrip")]
    LStrip(Box<Expr>),
    #[serde(rename = "rstrip")]
    RStrip(Box<Expr>),
    #[serde(rename = "isdigit")]
    IsDigit(Box<Expr>),
    #[serde(rename = "isalpha")]
    IsAlpha(Box<Expr>),
    #[serde(rename = "isalnum")]
    IsAlnum(Box<Expr>),
    #[serde(rename = "isspace")]
    IsSpace(Box<Expr>),
    #[serde(rename = "isupper")]
    IsUpper(Box<Expr>),
    #[serde(rename = "islower")]
    IsLower(Box<Expr>),
    #[serde(rename = "isascii")]
    IsAscii(Box<Expr>),
}
