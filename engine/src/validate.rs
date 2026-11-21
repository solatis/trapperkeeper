//! Checks a rule before it is stored or sent to a sensor. It checks the
//! rule's own fields, how deep the expression is and how long it prints
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
    /// The expression is more than `max` levels deep, where `max` is
    /// `MAX_DEPTH` and the top node is level 1. The error points at the
    /// first node that is too deep, in JSON order.
    TooDeep { max: usize },
    /// The expression prints as `len` characters of rule text. That is more
    /// than `max`, which is `MAX_TEXT_LENGTH`.
    TextTooLong { len: usize, max: usize },
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

/// Checks `e` and every node below it, and returns the first error. `at` is
/// the JSON pointer of `e`. `depth` is the level of `e`, where the top node
/// is level 1.
fn check_nodes(e: &Expr, at: &str, depth: usize) -> Result<(), Error> {
    // Stopping here also keeps this function from overflowing the stack on a
    // very deep tree, because it calls itself once per level.
    if depth > MAX_DEPTH {
        return refuse(at, Reason::TooDeep { max: MAX_DEPTH });
    }
    for (steps, child) in e.children() {
        // `steps` needs no escaping: it is made of JSON keys of the tree and
        // indexes, and none of them holds a `~` or a `/`.
        check_nodes(child, &format!("{at}/{steps}"), depth + 1)?;
    }
    Ok(())
}

/// Checks the id, the sample rate, the scope tags and the expression of
/// `rule`. For the expression it checks how deep it is and how long it
/// prints. It does not check that the parser could have built the tree
/// (ADR-0033).
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
