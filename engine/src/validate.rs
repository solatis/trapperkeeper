//! Validation of a rule's own fields, before the rule is stored or sent to a
//! sensor. It does not check the expression.

use serde::Serialize;

use crate::limits::{MAX_SCOPE_TAGS, MAX_TAG_LENGTH};
use crate::rule::Rule;

/// Why a part of a rule was refused.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "reason")]
pub enum Reason {
    SampleRateOutOfRange { sample_rate: f64 },
    RuleIdNotTimeOrdered { version: usize },
    TooManyTags { count: usize, max: usize },
    TagKeyLength { len: usize, max: usize },
    TagValueLength { len: usize, max: usize },
}

/// One refusal: where in the rule document, as a JSON pointer (RFC 6901),
/// and why.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Error {
    pub at: String,
    pub reason: Reason,
}

/// An object key as a reference token of a JSON pointer.
fn token(key: &str) -> String {
    // RFC 6901, 3: `~` is written `~0` and `/` is written `~1`. `~` goes
    // first, so the `~` of a new `~1` is not escaped again.
    key.replace('~', "~0").replace('/', "~1")
}

/// Checks the id, the sample rate and the scope tags of `rule` and returns
/// every refusal, so a caller fixes them in one pass. It accepts every
/// expression.
///
/// # Errors
///
/// The refusals, in document order, when there is at least one.
pub fn validate(rule: &Rule) -> Result<(), Vec<Error>> {
    let mut errors = Vec::new();
    let mut refuse = |at: String, reason| errors.push(Error { at, reason });

    // The id must be a version 7 UUID, the time-ordered kind the server
    // issues (ADR-0020). The nil UUID is version 0 and fails the same check.
    let version = rule.rule_id.get_version_num();
    if version != 7 {
        refuse(
            "/rule_id".to_owned(),
            Reason::RuleIdNotTimeOrdered { version },
        );
    }
    // A rate of 0 is refused because a rule that samples nothing is a
    // mistake. A NaN fails both comparisons, so it is refused too.
    let sample_rate = rule.sample_rate;
    if !(sample_rate > 0.0 && sample_rate <= 1.0) {
        refuse(
            "/sample_rate".to_owned(),
            Reason::SampleRateOutOfRange { sample_rate },
        );
    }
    // The caps keep the seen-scope index finite (ADR-0034). A key or a value
    // of zero length addresses nothing on any sensor.
    let count = rule.scope.len();
    if count > MAX_SCOPE_TAGS {
        let max = MAX_SCOPE_TAGS;
        refuse("/scope".to_owned(), Reason::TooManyTags { count, max });
    }
    let max = MAX_TAG_LENGTH;
    for (key, value) in &rule.scope {
        // Both refusals point at the tag, because a JSON pointer cannot
        // point at a key. The reason says which of the two is wrong.
        let at = format!("/scope/{}", token(key));
        // The caps count characters, which is what a person types and reads.
        // A byte count would cap a tag in a non-Latin script sooner.
        let len = key.chars().count();
        if !(1..=max).contains(&len) {
            refuse(at.clone(), Reason::TagKeyLength { len, max });
        }
        // A tag whose key and value are both wrong gets both refusals.
        let len = value.chars().count();
        if !(1..=max).contains(&len) {
            refuse(at, Reason::TagValueLength { len, max });
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
