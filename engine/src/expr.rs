//! The expression tree of the rule language and its JSON form. ADR-0041
//! decides the kinds of expression and the closed list of functions and
//! methods, and ADR-0033 decides the form of the tree.

use serde::{Deserialize, Serialize};

/// One step of a path: a key into an object, or an index into a list. In
/// JSON a key is a text and an index is a number.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Segment {
    Key(String),
    // The width of an index is a choice that no measurement backs. A path
    // has no negative index, because the parser refuses one (ADR-0041).
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
/// `is not None` have kinds of their own. A node is written as a JSON object
/// whose one key names its kind, except `None`, which has no operand and is
/// the bare string "none".
///
/// The parser builds `And` and `Or` with two or more operands in written
/// order, and it splices an operand of the same kind into the list, so
/// `(a and b) and c` is one node of three. The JSON form enforces neither
/// property and reads an `and` of any length.
///
/// Each comparison, from `Eq` to `IsNotNone`, holds its left and its right
/// operand, except `IsNone` and `IsNotNone`, which hold the one value they
/// test. An `Eq` or a `Ne` with the literal `None` as an operand tests for a
/// null, as `IsNone` and `IsNotNone` do. A value that is not there is a null
/// too (ADR-0005).
///
/// `Any` and `All` are the quantifiers `any` and `all`, which an author
/// writes as `any(test for name in list)`.
///
/// The JSON key of a function, from `Len` to `ReFullmatch`, is the name an
/// author types, with `_` for the dot, so `re.search` has the key
/// `re_search`. The pattern of `ReSearch`, `ReMatch` and `ReFullmatch` is a
/// literal text rather than an expression, so the server can check it when
/// the rule is created.
///
/// The first operand of a method, from `StartsWith` to `IsAscii`, is the
/// text the method is called on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
// A node with a key it does not know, or with a property its kind does not
// have, fails the parse.
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Expr {
    // Literals.
    None,
    Bool(bool),
    Int(i64),
    /// JSON has no form for a NaN or an infinity, so a float in a tree must
    /// be finite. The parser builds no other float, and `validate` does not
    /// check a tree that other code builds.
    Float(f64),
    Text(String),
    List(Vec<Expr>),

    // Fields.
    /// The path starts at the record.
    Field(Vec<Segment>),
    /// The path starts at the element that an enclosing `Any` or `All` names.
    /// An empty path is the element itself. The JSON form does not check that
    /// a quantifier names the element.
    Element {
        name: String,
        path: Vec<Segment>,
    },

    // Logic.
    Not(Box<Expr>),
    And(Vec<Expr>),
    Or(Vec<Expr>),

    // Comparisons.
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

    /// The conditional, `then if test else otherwise`.
    If {
        test: Box<Expr>,
        then: Box<Expr>,
        // `else` is a keyword of Rust, so the field has another name and
        // JSON keeps `else`.
        #[serde(rename = "else")]
        otherwise: Box<Expr>,
    },

    // The quantifiers.
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

    // The functions.
    Len(Box<Expr>),
    #[serde(rename = "isinstance")]
    IsInstance {
        value: Box<Expr>,
        types: Vec<TypeName>,
    },
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

    // The methods.
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

/// The operands of one node, in the form that JSON writes them. `R` is
/// `&Expr` or `&mut Expr`.
pub(crate) enum Operands<R> {
    /// A literal, a field or an element. It has no operand.
    Leaf,
    /// One operand, which JSON writes directly under the key.
    One(R),
    /// Operands that JSON writes as an array under the key.
    Listed(Vec<R>),
    /// Operands that JSON writes as named properties under the key.
    Named(Vec<(&'static str, R)>),
}

// The one table of the tree: a row per kind, with its JSON key and its
// operands. `Expr::parts` and `Expr::parts_mut` both expand it, because Rust
// cannot share one `match` between `&self` and `&mut self`. `$unbox` and
// `$iter` are the shared or the mutable way to reach an operand.
//
// serde derives the keys from the attributes on `Expr` and cannot be asked
// for them, so they are written again here. The property
// `a_node_past_the_depth_cap_is_refused_at_its_place` in `tests/model.rs`
// fails when a key or a property name here differs from serde's JSON. It
// covers every kind that has an operand. The key of a leaf is never used in
// a pointer.
//
// The types of `IsInstance` and the pattern of a pattern function are not
// nodes, so they are not operands.
macro_rules! parts {
    ($e:expr, $unbox:ident, $iter:ident) => {
        match $e {
            Expr::None => ("none", Operands::Leaf),
            Expr::Bool(_) => ("bool", Operands::Leaf),
            Expr::Int(_) => ("int", Operands::Leaf),
            Expr::Float(_) => ("float", Operands::Leaf),
            Expr::Text(_) => ("text", Operands::Leaf),
            Expr::Field(_) => ("field", Operands::Leaf),
            Expr::Element { .. } => ("element", Operands::Leaf),
            Expr::List(items) => ("list", Operands::Listed(items.$iter().collect())),
            Expr::And(items) => ("and", Operands::Listed(items.$iter().collect())),
            Expr::Or(items) => ("or", Operands::Listed(items.$iter().collect())),
            Expr::Not(x) => ("not", Operands::One(x.$unbox())),
            Expr::IsNone(x) => ("is_none", Operands::One(x.$unbox())),
            Expr::IsNotNone(x) => ("is_not_none", Operands::One(x.$unbox())),
            Expr::Neg(x) => ("neg", Operands::One(x.$unbox())),
            Expr::Len(x) => ("len", Operands::One(x.$unbox())),
            Expr::Upper(x) => ("upper", Operands::One(x.$unbox())),
            Expr::Lower(x) => ("lower", Operands::One(x.$unbox())),
            Expr::Strip(x) => ("strip", Operands::One(x.$unbox())),
            Expr::LStrip(x) => ("lstrip", Operands::One(x.$unbox())),
            Expr::RStrip(x) => ("rstrip", Operands::One(x.$unbox())),
            Expr::IsDigit(x) => ("isdigit", Operands::One(x.$unbox())),
            Expr::IsAlpha(x) => ("isalpha", Operands::One(x.$unbox())),
            Expr::IsAlnum(x) => ("isalnum", Operands::One(x.$unbox())),
            Expr::IsSpace(x) => ("isspace", Operands::One(x.$unbox())),
            Expr::IsUpper(x) => ("isupper", Operands::One(x.$unbox())),
            Expr::IsLower(x) => ("islower", Operands::One(x.$unbox())),
            Expr::IsAscii(x) => ("isascii", Operands::One(x.$unbox())),
            Expr::Eq(l, r) => ("eq", Operands::Listed(vec![l.$unbox(), r.$unbox()])),
            Expr::Ne(l, r) => ("ne", Operands::Listed(vec![l.$unbox(), r.$unbox()])),
            Expr::Lt(l, r) => ("lt", Operands::Listed(vec![l.$unbox(), r.$unbox()])),
            Expr::Le(l, r) => ("le", Operands::Listed(vec![l.$unbox(), r.$unbox()])),
            Expr::Gt(l, r) => ("gt", Operands::Listed(vec![l.$unbox(), r.$unbox()])),
            Expr::Ge(l, r) => ("ge", Operands::Listed(vec![l.$unbox(), r.$unbox()])),
            Expr::In(l, r) => ("in", Operands::Listed(vec![l.$unbox(), r.$unbox()])),
            Expr::NotIn(l, r) => ("not_in", Operands::Listed(vec![l.$unbox(), r.$unbox()])),
            Expr::Add(l, r) => ("add", Operands::Listed(vec![l.$unbox(), r.$unbox()])),
            Expr::Sub(l, r) => ("sub", Operands::Listed(vec![l.$unbox(), r.$unbox()])),
            Expr::Mul(l, r) => ("mul", Operands::Listed(vec![l.$unbox(), r.$unbox()])),
            Expr::Div(l, r) => ("div", Operands::Listed(vec![l.$unbox(), r.$unbox()])),
            Expr::Mod(l, r) => ("mod", Operands::Listed(vec![l.$unbox(), r.$unbox()])),
            Expr::StartsWith(l, r) => {
                ("startswith", Operands::Listed(vec![l.$unbox(), r.$unbox()]))
            }
            Expr::EndsWith(l, r) => ("endswith", Operands::Listed(vec![l.$unbox(), r.$unbox()])),
            Expr::If {
                test,
                then,
                otherwise,
            } => (
                "if",
                Operands::Named(vec![
                    ("test", test.$unbox()),
                    ("then", then.$unbox()),
                    ("else", otherwise.$unbox()),
                ]),
            ),
            Expr::Any { list, test, .. } => (
                "any",
                Operands::Named(vec![("list", list.$unbox()), ("test", test.$unbox())]),
            ),
            Expr::All { list, test, .. } => (
                "all",
                Operands::Named(vec![("list", list.$unbox()), ("test", test.$unbox())]),
            ),
            Expr::IsInstance { value, .. } => (
                "isinstance",
                Operands::Named(vec![("value", value.$unbox())]),
            ),
            Expr::ReSearch { text, .. } => {
                ("re_search", Operands::Named(vec![("text", text.$unbox())]))
            }
            Expr::ReMatch { text, .. } => {
                ("re_match", Operands::Named(vec![("text", text.$unbox())]))
            }
            Expr::ReFullmatch { text, .. } => (
                "re_fullmatch",
                Operands::Named(vec![("text", text.$unbox())]),
            ),
        }
    };
}

impl Expr {
    /// The JSON key of this node and its operands.
    fn parts(&self) -> (&'static str, Operands<&Expr>) {
        parts!(self, as_ref, iter)
    }

    /// Like `parts`, with operands that can be changed.
    fn parts_mut(&mut self) -> (&'static str, Operands<&mut Expr>) {
        parts!(self, as_mut, iter_mut)
    }

    /// The JSON key of this kind of node, such as `gt` for `Gt` and
    /// `re_search` for `ReSearch`.
    pub(crate) fn key(&self) -> &'static str {
        self.parts().0
    }

    /// The child nodes in JSON order. Each comes with its path from this
    /// node, in JSON pointer form, such as `gt/1` for the right side of a
    /// `Gt`.
    pub(crate) fn children(&self) -> Vec<(String, &Expr)> {
        let (key, operands) = self.parts();
        match operands {
            Operands::Leaf => Vec::new(),
            Operands::One(x) => vec![(key.to_owned(), x)],
            Operands::Listed(xs) => {
                let child = |(i, x)| (format!("{key}/{i}"), x);
                xs.into_iter().enumerate().map(child).collect()
            }
            Operands::Named(xs) => {
                let child = |(name, x)| (format!("{key}/{name}"), x);
                xs.into_iter().map(child).collect()
            }
        }
    }

    /// The child nodes in JSON order, for a caller that changes them.
    pub(crate) fn children_mut(&mut self) -> Vec<&mut Expr> {
        match self.parts_mut().1 {
            Operands::Leaf => Vec::new(),
            Operands::One(x) => vec![x],
            Operands::Listed(xs) => xs,
            Operands::Named(xs) => xs.into_iter().map(|(_, x)| x).collect(),
        }
    }
}
