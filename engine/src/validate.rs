//! Checks a rule before it is stored or sent to a sensor. It checks the
//! rule's own fields, how deep the expression is, how long it prints,
//! whether its patterns compile and where the literal `None` stands
//! (ADR-0033).

use std::fmt::{self, Write as _};

use regex::Regex;
use serde::Serialize;

use crate::expr::Expr;
use crate::limits::{MAX_DEPTH, MAX_SCOPE_TAGS, MAX_TAG_LENGTH, MAX_TEXT_LENGTH};
use crate::rule::Rule;

/// Why a part of a rule was refused.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "reason")]
pub enum Reason {
    /// The sample rate lies outside (0, 1] or is a NaN. `sample_rate` holds
    /// the refused value, which JSON writes as null when it is a NaN.
    SampleRateOutOfRange { sample_rate: f64 },
    /// The id is not a version 7 UUID. `version` holds the version number of
    /// the given UUID, 0 for the nil UUID.
    RuleIdNotTimeOrdered { version: usize },
    /// The scope holds `count` tags, which is more than `max`, the cap
    /// `MAX_SCOPE_TAGS`.
    TooManyTags { count: usize, max: usize },
    /// A tag key is empty or longer than `max`. `len` counts the characters
    /// of the key, and the accepted range is 1 to `max`.
    TagKeyLength { len: usize, max: usize },
    /// A tag value is empty or longer than `max`. `len` counts the characters
    /// of the value, and the accepted range is 1 to `max`.
    TagValueLength { len: usize, max: usize },
    /// The expression is more than `max` levels deep, where `max` is
    /// `MAX_DEPTH` and the top node is level 1. The error points at the
    /// first node that is too deep, in JSON order.
    TooDeep { max: usize },
    /// The expression prints as `len` characters of rule text. That is more
    /// than `max`, which is `MAX_TEXT_LENGTH`.
    TextTooLong { len: usize, max: usize },
    /// The pattern of `re.search`, `re.match` or `re.fullmatch` does not
    /// compile. `message` is the `regex` crate's own error text. For a syntax
    /// error it has several lines and marks the place in the pattern. For a
    /// pattern that compiles too large it is one line.
    BadPattern { message: String },
    /// The literal `None` is used where it can never do anything useful: next
    /// to `<`, `<=`, `>` or `>=`, next to an arithmetic operator, under a
    /// minus, or on the right of `in` or `not in` (ADR-0005). The error points
    /// at that `None`.
    NoneOperand,
}

/// One refusal: where in the rule document, as a JSON pointer (RFC 6901),
/// and why. `validate` returns the first one it finds.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Error {
    pub at: String,
    pub reason: Reason,
}

/// An object key as a reference token of a JSON pointer.
fn token(key: &str) -> String {
    // Section 3 of RFC 6901 writes `~` as `~0` and `/` as `~1`. The `~`
    // replacement runs first, so the `~` of a `~1` that the second
    // replacement writes is not escaped again.
    key.replace('~', "~0").replace('/', "~1")
}

/// Returns the error for `reason` at the JSON pointer `at`.
fn refuse(at: &str, reason: Reason) -> Result<(), Error> {
    Err(Error {
        at: at.to_owned(),
        reason,
    })
}

/// A writer that counts the characters it gets and throws them away.
struct Counter(usize);

impl fmt::Write for Counter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        // `MAX_TEXT_LENGTH` counts Unicode scalar values, so this does too.
        self.0 += s.chars().count();
        Ok(())
    }
}

/// How many characters `e` takes when printed as rule text.
fn printed_length(e: &Expr) -> usize {
    let mut counter = Counter(0);
    // Printing into the counter means the text is never held in memory. The
    // counter keeps going past the cap, so the error can say how long the
    // text really is, the way `TooManyTags` gives its count.
    // `Counter` never fails, so the result is dropped.
    let _ = write!(counter, "{e}");
    counter.0
}

/// Which operands of `e` may not be the literal `None`, as indexes into
/// `e.children()`.
fn operands_without_none(e: &Expr) -> &'static [usize] {
    match e {
        // `x < None` is unknown and `None + 1` is a null, for every record.
        // So neither side may be the literal (ADR-0005).
        Expr::Lt(..)
        | Expr::Le(..)
        | Expr::Gt(..)
        | Expr::Ge(..)
        | Expr::Add(..)
        | Expr::Sub(..)
        | Expr::Mul(..)
        | Expr::Div(..)
        | Expr::Mod(..) => &[0, 1],
        // `-None` is a null for every record.
        Expr::Neg(_) => &[0],
        // Only the right side. `None in x` is fine: it is true when the list
        // `x` holds a null (ADR-0005).
        Expr::In(..) | Expr::NotIn(..) => &[1],
        // `==` and `!=` with `None` test for a null. Everywhere else `None`
        // is just a missing value. `len(None)` and `None.upper()` are left
        // for the type check that comes with the planner (ADR-0005).
        _ => &[],
    }
}

/// Checks `e` and every node below it, and returns the first error. For each
/// node it checks the depth, the pattern if it has one, and that no operand
/// is a `None` it may not hold. `at` is the JSON pointer of `e`. `depth` is
/// the level of `e`, where the top node is level 1.
fn check_nodes(e: &Expr, at: &str, depth: usize) -> Result<(), Error> {
    // Stopping here also keeps this function from overflowing the stack on a
    // very deep tree, because it calls itself once per level.
    if depth > MAX_DEPTH {
        return refuse(at, Reason::TooDeep { max: MAX_DEPTH });
    }
    // All three pattern functions call their pattern `pattern`, so one
    // check covers them.
    if let Expr::ReSearch { pattern, .. }
    | Expr::ReMatch { pattern, .. }
    | Expr::ReFullmatch { pattern, .. } = e
        // Compiled with the crate's defaults, the same way a sensor will
        // compile it. That includes the crate's own size cap (ADR-0041).
        && let Err(error) = Regex::new(pattern)
    {
        // The error points at the pattern itself, which sits in the node
        // under the node's key and `pattern`.
        let at = format!("{at}/{}/pattern", e.key());
        let message = error.to_string();
        return refuse(&at, Reason::BadPattern { message });
    }
    let without_none = operands_without_none(e);
    for (index, (steps, child)) in e.children().into_iter().enumerate() {
        // `steps` needs no escaping: it is made of JSON keys of the tree and
        // indexes, and none of them holds a `~` or a `/`.
        let at = format!("{at}/{steps}");
        check_nodes(child, &at, depth + 1)?;
        // This comes after the child's own checks, so a `None` that is also
        // too deep gets `TooDeep`. Only the literal is refused: a field that
        // happens to hold a null is data.
        if matches!(child, Expr::None) && without_none.contains(&index) {
            return refuse(&at, Reason::NoneOperand);
        }
    }
    Ok(())
}

/// Checks the id, the sample rate, the scope tags and the expression of
/// `rule`. For the expression it checks how deep it is, whether its patterns
/// compile, where the literal `None` stands and how long it prints. It does
/// not check that the parser could have built the tree, such as whether an
/// `and` has two operands (ADR-0033).
///
/// # Errors
///
/// The first error it finds. It checks in the order of the rule's JSON: the
/// id, the sample rate, the scope, each tag in key order with the key before
/// the value, and then the expression. In the expression a node comes before
/// its children, and the printed length comes last.
pub fn validate(rule: &Rule) -> Result<(), Error> {
    // The checks run in the order of the rule's JSON, and the first one that
    // fails returns. The fixed order means a rule with several mistakes
    // always gets the same error. The printed length comes last, and the
    // note there says why.
    let version = rule.rule_id.get_version_num();
    // The server issues version 7 UUIDs, and a format that admits any
    // version would admit ids that the server never issues (ADR-0033). The
    // nil UUID is version 0 and fails the same check.
    if version != 7 {
        return refuse("/rule_id", Reason::RuleIdNotTimeOrdered { version });
    }
    let sample_rate = rule.sample_rate;
    // A rate of 0 is refused because a rule that samples nothing is a
    // mistake. A NaN fails both comparisons, so it is refused too.
    if !(sample_rate > 0.0 && sample_rate <= 1.0) {
        return refuse("/sample_rate", Reason::SampleRateOutOfRange { sample_rate });
    }
    let count = rule.scope.len();
    if count > MAX_SCOPE_TAGS {
        let max = MAX_SCOPE_TAGS;
        return refuse("/scope", Reason::TooManyTags { count, max });
    }
    let max = MAX_TAG_LENGTH;
    // The scope is a `BTreeMap`, so the loop visits the tags in key order.
    for (key, value) in &rule.scope {
        // Both refusals point at the tag, because a JSON pointer cannot
        // point at a key. The reason says which of the two is wrong.
        let at = format!("/scope/{}", token(key));
        // The caps count Unicode scalar values, which ADR-0034 calls
        // characters. A byte count would cap a tag in a non-Latin script
        // sooner.
        let len = key.chars().count();
        // A key or a value of zero length addresses nothing on any sensor,
        // because a sensor's own tags are 1 to `MAX_TAG_LENGTH` characters
        // too (ADR-0034).
        if !(1..=max).contains(&len) {
            return refuse(&at, Reason::TagKeyLength { len, max });
        }
        let len = value.chars().count();
        if !(1..=max).contains(&len) {
            return refuse(&at, Reason::TagValueLength { len, max });
        }
    }
    // The top node is level 1.
    check_nodes(&rule.expression, "/expression", 1)?;
    // The printer calls itself once per level, so it must not see a very
    // deep tree. The walk above has already refused those.
    let len = printed_length(&rule.expression);
    if len > MAX_TEXT_LENGTH {
        let max = MAX_TEXT_LENGTH;
        return refuse("/expression", Reason::TextTooLong { len, max });
    }
    Ok(())
}
