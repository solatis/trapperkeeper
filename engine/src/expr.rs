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
/// The JSON key of a function, from `Len` to `IsNan`, is the name an author
/// types, with `_` for the dot, so `re.search` has the key `re_search`. The
/// pattern of `ReSearch`, `ReMatch` and `ReFullmatch` is a literal text
/// rather than an expression, so the server can check it when the rule is
/// created.
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
    #[serde(rename = "math_isnan")]
    IsNan(Box<Expr>),

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

impl Expr {
    /// The key that names the kind of this node in its JSON form, as `gt` for
    /// `Gt` and `re_search` for `ReSearch`. For `None` it is `none`, the bare
    /// string that `None` is written as.
    pub(crate) fn key(&self) -> &'static str {
        // serde derives these names from the attributes on `Expr` and gives
        // no way to ask for the name of a variant. The property
        // `a_node_past_the_depth_cap_is_refused_at_its_place` in
        // `tests/model.rs` puts nodes of every kind that holds a node above a
        // defect. It fails when the pointer that `children` builds from these
        // names differs from the place of the defect in serde's own JSON. The
        // name of a kind without a node appears in no pointer.
        match self {
            Expr::None => "none",
            Expr::Bool(_) => "bool",
            Expr::Int(_) => "int",
            Expr::Float(_) => "float",
            Expr::Text(_) => "text",
            Expr::List(_) => "list",
            Expr::Field(_) => "field",
            Expr::Element { .. } => "element",
            Expr::Not(_) => "not",
            Expr::And(_) => "and",
            Expr::Or(_) => "or",
            Expr::Eq(..) => "eq",
            Expr::Ne(..) => "ne",
            Expr::Lt(..) => "lt",
            Expr::Le(..) => "le",
            Expr::Gt(..) => "gt",
            Expr::Ge(..) => "ge",
            Expr::In(..) => "in",
            Expr::NotIn(..) => "not_in",
            Expr::IsNone(_) => "is_none",
            Expr::IsNotNone(_) => "is_not_none",
            Expr::Add(..) => "add",
            Expr::Sub(..) => "sub",
            Expr::Mul(..) => "mul",
            Expr::Div(..) => "div",
            Expr::Mod(..) => "mod",
            Expr::Neg(_) => "neg",
            Expr::If { .. } => "if",
            Expr::Any { .. } => "any",
            Expr::All { .. } => "all",
            Expr::Len(_) => "len",
            Expr::IsInstance { .. } => "isinstance",
            Expr::ReSearch { .. } => "re_search",
            Expr::ReMatch { .. } => "re_match",
            Expr::ReFullmatch { .. } => "re_fullmatch",
            Expr::IsNan(_) => "math_isnan",
            Expr::StartsWith(..) => "startswith",
            Expr::EndsWith(..) => "endswith",
            Expr::Upper(_) => "upper",
            Expr::Lower(_) => "lower",
            Expr::Strip(_) => "strip",
            Expr::LStrip(_) => "lstrip",
            Expr::RStrip(_) => "rstrip",
            Expr::IsDigit(_) => "isdigit",
            Expr::IsAlpha(_) => "isalpha",
            Expr::IsAlnum(_) => "isalnum",
            Expr::IsSpace(_) => "isspace",
            Expr::IsUpper(_) => "isupper",
            Expr::IsLower(_) => "islower",
            Expr::IsAscii(_) => "isascii",
        }
    }

    /// The nodes directly under this one, in the order of the JSON form. Each
    /// comes with the steps of a JSON pointer that lead to it from this node,
    /// as `gt/1` for the right operand of a `Gt`.
    pub(crate) fn children(&self) -> Vec<(String, &Expr)> {
        let key = self.key();
        match self {
            Expr::None
            | Expr::Bool(_)
            | Expr::Int(_)
            | Expr::Float(_)
            | Expr::Text(_)
            | Expr::Field(_)
            | Expr::Element { .. } => Vec::new(),
            // JSON writes the operands as an array, so the steps are the key
            // and the index.
            Expr::List(items) | Expr::And(items) | Expr::Or(items) => items
                .iter()
                .enumerate()
                .map(|(i, x)| (format!("{key}/{i}"), x))
                .collect(),
            // The JSON of such a node is an object whose one value is the
            // operand, so the step is the key alone.
            Expr::Not(x)
            | Expr::IsNone(x)
            | Expr::IsNotNone(x)
            | Expr::Neg(x)
            | Expr::Len(x)
            | Expr::IsNan(x)
            | Expr::Upper(x)
            | Expr::Lower(x)
            | Expr::Strip(x)
            | Expr::LStrip(x)
            | Expr::RStrip(x)
            | Expr::IsDigit(x)
            | Expr::IsAlpha(x)
            | Expr::IsAlnum(x)
            | Expr::IsSpace(x)
            | Expr::IsUpper(x)
            | Expr::IsLower(x)
            | Expr::IsAscii(x) => vec![(key.to_owned(), x)],
            // The types of `IsInstance` and the pattern of a pattern function
            // are not nodes, so the value and the text are the only children.
            // JSON writes these kinds as an object of named properties.
            Expr::IsInstance { value: x, .. } => vec![(format!("{key}/value"), x)],
            Expr::ReSearch { text: x, .. }
            | Expr::ReMatch { text: x, .. }
            | Expr::ReFullmatch { text: x, .. } => vec![(format!("{key}/text"), x)],
            // As for a list, the steps are the key and the index.
            Expr::Eq(l, r)
            | Expr::Ne(l, r)
            | Expr::Lt(l, r)
            | Expr::Le(l, r)
            | Expr::Gt(l, r)
            | Expr::Ge(l, r)
            | Expr::In(l, r)
            | Expr::NotIn(l, r)
            | Expr::Add(l, r)
            | Expr::Sub(l, r)
            | Expr::Mul(l, r)
            | Expr::Div(l, r)
            | Expr::Mod(l, r)
            | Expr::StartsWith(l, r)
            | Expr::EndsWith(l, r) => vec![(format!("{key}/0"), l), (format!("{key}/1"), r)],
            // JSON writes the conditional and the quantifiers as an object of
            // named properties, in the order of the fields of the variant.
            Expr::If {
                test,
                then,
                otherwise,
            } => vec![
                (format!("{key}/test"), test),
                (format!("{key}/then"), then),
                (format!("{key}/else"), otherwise),
            ],
            Expr::Any { list, test, .. } | Expr::All { list, test, .. } => {
                vec![(format!("{key}/list"), list), (format!("{key}/test"), test)]
            }
        }
    }

    /// The nodes directly under this one, in the order of the rule text.
    /// `children` gives the same nodes in the order of the JSON form, with the
    /// steps of their pointers.
    pub(crate) fn children_mut(&mut self) -> Vec<&mut Expr> {
        // `resolve` in `text/grammar.rs` is the only caller. It resolves each
        // child on its own and handles `Any` and `All` itself, so no result
        // of it depends on the order.
        match self {
            // `None`, a boolean, a number and a text hold no node. A field
            // and an element hold a path of segments, and a segment is not a
            // node.
            Expr::None
            | Expr::Bool(_)
            | Expr::Int(_)
            | Expr::Float(_)
            | Expr::Text(_)
            | Expr::Field(_)
            | Expr::Element { .. } => Vec::new(),
            Expr::List(items) | Expr::And(items) | Expr::Or(items) => items.iter_mut().collect(),
            // The arm holds the kinds with one child. The types of
            // `IsInstance` and the pattern of a pattern function are not
            // nodes, so only the value and the text are children.
            Expr::Not(x)
            | Expr::IsNone(x)
            | Expr::IsNotNone(x)
            | Expr::Neg(x)
            | Expr::Len(x)
            | Expr::IsNan(x)
            | Expr::Upper(x)
            | Expr::Lower(x)
            | Expr::Strip(x)
            | Expr::LStrip(x)
            | Expr::RStrip(x)
            | Expr::IsDigit(x)
            | Expr::IsAlpha(x)
            | Expr::IsAlnum(x)
            | Expr::IsSpace(x)
            | Expr::IsUpper(x)
            | Expr::IsLower(x)
            | Expr::IsAscii(x)
            | Expr::IsInstance { value: x, .. }
            | Expr::ReSearch { text: x, .. }
            | Expr::ReMatch { text: x, .. }
            | Expr::ReFullmatch { text: x, .. } => vec![x],
            // Two nodes, the left before the right. A method's receiver
            // stands before its argument.
            Expr::Eq(l, r)
            | Expr::Ne(l, r)
            | Expr::Lt(l, r)
            | Expr::Le(l, r)
            | Expr::Gt(l, r)
            | Expr::Ge(l, r)
            | Expr::In(l, r)
            | Expr::NotIn(l, r)
            | Expr::Add(l, r)
            | Expr::Sub(l, r)
            | Expr::Mul(l, r)
            | Expr::Div(l, r)
            | Expr::Mod(l, r)
            | Expr::StartsWith(l, r)
            | Expr::EndsWith(l, r) => vec![l, r],
            // In the text the test of a quantifier stands before its list.
            // The arm does not bind the name, so a caller that follows the
            // scope handles `Any` and `All` itself, as `resolve` does.
            Expr::Any { list, test, .. } | Expr::All { list, test, .. } => vec![test, list],
            // In the text the first branch stands before the test.
            Expr::If {
                test,
                then,
                otherwise,
            } => vec![then, test, otherwise],
        }
    }
}
