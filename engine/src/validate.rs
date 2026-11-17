//! Validation of a rule's own fields, before the rule is stored or sent to a
//! sensor. It does not check the expression.

use serde::Serialize;

use crate::limits::{MAX_SCOPE_TAGS, MAX_TAG_LENGTH};
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

/// Checks the id, the sample rate and the scope tags of `rule`. It accepts
/// every expression.
///
/// # Errors
///
/// The first refusal, in the order in which the JSON form of a rule writes
/// the fields: the id, then the sample rate, then the scope, then each tag
/// in key order, with a key before its value.
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
    Ok(())
}
