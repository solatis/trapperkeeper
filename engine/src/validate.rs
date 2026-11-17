//! Validation of a rule before it is stored or sent to a sensor: the rule's
//! own fields, and the depth and the printed length of its expression
//! (ADR-0033).

use std::fmt::{self, Write as _};

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
    /// The way from the top of the expression down to a node passes more than
    /// `max` nodes, the cap `MAX_DEPTH`, with the top and the node counted.
    /// The refusal points at the node past the cap that the walk meets first,
    /// in the order of the JSON form.
    TooDeep { max: usize },
    /// The expression prints as `len` characters of rule text, which is more
    /// than `max`, the cap `MAX_TEXT_LENGTH`.
    TextTooLong { len: usize, max: usize },
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

/// Counts the characters that are written to it and keeps none of them.
struct Counter(usize);

impl fmt::Write for Counter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        // Characters are Unicode scalar values, as `MAX_TEXT_LENGTH` counts
        // them.
        self.0 += s.chars().count();
        Ok(())
    }
}

/// The number of characters of `e` printed as rule text.
fn printed_length(e: &Expr) -> usize {
    let mut counter = Counter(0);
    // The printer writes into the counter, so a text of any length is counted
    // and never held in memory. The counter does not stop at the cap, so
    // `TextTooLong` holds the whole length, as `TooManyTags` holds its count,
    // and the author sees how far over the rule is.
    // `Counter` never returns an error, so the write cannot fail and its
    // result is dropped.
    let _ = write!(counter, "{e}");
    counter.0
}

/// Checks the depth of `e` and then of every node under it, and returns the
/// first refusal. `at` is the JSON pointer of `e`, and `depth` counts the nodes
/// from the top of the expression down to `e`.
fn check_nodes(e: &Expr, at: &str, depth: usize) -> Result<(), Error> {
    // The walk refuses the node and does not go further down. It calls itself
    // once per level, so the cap also bounds its own stack on a tree of any
    // depth.
    if depth > MAX_DEPTH {
        return refuse(at, Reason::TooDeep { max: MAX_DEPTH });
    }
    for (steps, child) in e.children() {
        // The steps are keys of the tree's JSON form and indexes, joined by
        // `/`. No key and no index holds a `~` or a `/` that would need
        // escaping.
        check_nodes(child, &format!("{at}/{steps}"), depth + 1)?;
    }
    Ok(())
}

/// Checks the id, the sample rate, the scope tags and the expression of
/// `rule`: the depth of the expression and the length of its printed text.
/// It does not test whether the parser could have built the expression
/// (ADR-0033).
///
/// # Errors
///
/// The first refusal, in the order in which the JSON form of a rule writes
/// the fields: the id, then the sample rate, then the scope, then each tag
/// in key order, with a key before its value, then the expression, with a
/// node before its children, and last the length of its printed text.
pub fn validate(rule: &Rule) -> Result<(), Error> {
    // `validate` runs its checks in the order in which the JSON form of a
    // rule holds the fields, and it returns the refusal of the first check
    // that fails. The fixed order gives a rule with several defects the same
    // answer every time. The check of the printed text comes last, and the
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
    // The top of the expression is the first node, so its depth is 1.
    check_nodes(&rule.expression, "/expression", 1)?;
    // The printer calls itself once per level, so it runs after the walk,
    // which has refused every tree deeper than `MAX_DEPTH`.
    let len = printed_length(&rule.expression);
    if len > MAX_TEXT_LENGTH {
        let max = MAX_TEXT_LENGTH;
        return refuse("/expression", Reason::TextTooLong { len, max });
    }
    Ok(())
}
