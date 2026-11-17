//! Validation of a rule before it is stored or sent to a sensor: the rule's
//! own fields, and the depth of its expression (ADR-0033).

use serde::Serialize;

use crate::expr::Expr;
use crate::limits::{MAX_DEPTH, MAX_SCOPE_TAGS, MAX_TAG_LENGTH};
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
    /// A node lies more than `max` nodes below the top of the expression, the
    /// cap `MAX_DEPTH`. The refusal points at that node.
    TooDeep { max: usize },
}

/// One refusal: where in the rule document, as a JSON pointer (RFC 6901),
/// and why. `validate` returns the first refusal of a rule.
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

/// The refusal for `reason` at the JSON pointer `at`.
fn refuse(at: &str, reason: Reason) -> Result<(), Error> {
    Err(Error {
        at: at.to_owned(),
        reason,
    })
}

/// Checks `e` and then every node under it, and returns the first refusal.
/// `at` is the JSON pointer of `e`, and `depth` counts the nodes from the
/// top of the expression down to `e`.
fn check_nodes(e: &Expr, at: &str, depth: usize) -> Result<(), Error> {
    // The walk refuses the node and does not go further down. It calls itself
    // once per level, so the cap also bounds its own stack on a tree of any
    // depth.
    if depth > MAX_DEPTH {
        return refuse(at, Reason::TooDeep { max: MAX_DEPTH });
    }
    for (steps, child) in e.children() {
        // Each step is a key of the tree's JSON form or an index, so none
        // holds a `~` or a `/` that would need escaping.
        check_nodes(child, &format!("{at}/{steps}"), depth + 1)?;
    }
    Ok(())
}

/// Checks the id, the sample rate, the scope tags and the expression of
/// `rule`. It does not test whether the parser could have built the
/// expression (ADR-0033).
///
/// # Errors
///
/// The first refusal, in the order in which the JSON form of a rule writes
/// the fields: the id, then the sample rate, then the scope, then each tag
/// in key order, with a key before its value, then the expression, with a
/// node before its children.
pub fn validate(rule: &Rule) -> Result<(), Error> {
    // `validate` runs its checks in the order in which the JSON form of a
    // rule holds the fields, and it returns the refusal of the first check
    // that fails. The fixed order gives a rule with several defects the same
    // answer every time.
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
    // The top of the expression is the first node, so its depth is 1.
    check_nodes(&rule.expression, "/expression", 1)
}
