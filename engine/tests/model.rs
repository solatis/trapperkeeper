//! Tests of the JSON form of a rule and of `validate`, which returns the
//! first refusal of a rule, as properties over generated rules. ADR-0018
//! prefers property tests.

mod common;

use std::collections::BTreeMap;

use proptest::prelude::*;
use proptest::sample::select;
use trapperkeeper_engine::limits::{MAX_SCOPE_TAGS, MAX_TAG_LENGTH};
use trapperkeeper_engine::{Error, Reason, Rule, validate};
use uuid::Uuid;

use common::rule;

/// One way to break a valid rule.
#[derive(Debug, Clone, Copy)]
enum Defect {
    SampleRateZero,
    SampleRateAboveOne,
    SampleRateNan,
    NilRuleId,
    RandomRuleId,
    TooManyTags,
    TagKeyEmpty,
    TagKeyTooLong,
    TagValueTooLong,
}

/// Every variant of `Defect`. The defect property draws its defect from this
/// array alone. The compiler checks the match in `apply` for completeness and
/// does not check this array, so a variant that is missing here is never
/// tested.
const DEFECTS: [Defect; 9] = [
    Defect::SampleRateZero,
    Defect::SampleRateAboveOne,
    Defect::SampleRateNan,
    Defect::NilRuleId,
    Defect::RandomRuleId,
    Defect::TooManyTags,
    Defect::TagKeyEmpty,
    Defect::TagKeyTooLong,
    Defect::TagValueTooLong,
];

/// The scope with one tag.
fn one_tag(key: &str, value: &str) -> BTreeMap<String, String> {
    BTreeMap::from([(key.to_owned(), value.to_owned())])
}

/// Changes `rule` by the defect `d` and returns the JSON pointer and the
/// reason of the refusal that `validate` must return. Every defect of the
/// scope replaces the whole scope, because the generated scope may already
/// hold `MAX_SCOPE_TAGS` tags, and with one more tag `validate` would refuse
/// the scope as too many first.
fn apply(d: Defect, rule: &mut Rule) -> (String, Reason) {
    let too_long = MAX_TAG_LENGTH + 1;

    match d {
        Defect::SampleRateZero | Defect::SampleRateAboveOne | Defect::SampleRateNan => {
            let sample_rate = match d {
                Defect::SampleRateZero => 0.0,
                // `1.0 + f64::EPSILON` is the smallest f64 above 1, so the
                // rate sits directly past the inclusive upper bound.
                Defect::SampleRateAboveOne => 1.0 + f64::EPSILON,
                // The outer arm admits three defects, so `_` is
                // `SampleRateNan` here. Naming it would make this match
                // non-exhaustive, because the compiler does not know that
                // the outer arm admits only three variants.
                _ => f64::NAN,
            };
            rule.sample_rate = sample_rate;
            (
                "/sample_rate".to_owned(),
                Reason::SampleRateOutOfRange { sample_rate },
            )
        }
        Defect::NilRuleId => {
            rule.rule_id = Uuid::nil();
            // `get_version_num` reports 0 for the nil UUID.
            let version = 0;
            (
                "/rule_id".to_owned(),
                Reason::RuleIdNotTimeOrdered { version },
            )
        }
        Defect::RandomRuleId => {
            // The id is a well-formed version 4 UUID, so the refusal concerns
            // the version alone. `from_random_bytes` sets the version and
            // variant bits over the given bytes, so the fill byte is arbitrary.
            rule.rule_id = uuid::Builder::from_random_bytes([0x5a; 16]).into_uuid();
            let version = 4;
            (
                "/rule_id".to_owned(),
                Reason::RuleIdNotTimeOrdered { version },
            )
        }
        Defect::TooManyTags => {
            let (count, max) = (MAX_SCOPE_TAGS + 1, MAX_SCOPE_TAGS);
            rule.scope = (0..count).map(|i| (format!("k{i}"), "v".into())).collect();
            ("/scope".to_owned(), Reason::TooManyTags { count, max })
        }
        Defect::TagKeyEmpty => {
            rule.scope = one_tag("", "v");
            let (len, max) = (0, MAX_TAG_LENGTH);
            // The empty key is an empty reference token, so the JSON pointer
            // of its tag is `/scope` followed by a slash and nothing after it.
            ("/scope/".to_owned(), Reason::TagKeyLength { len, max })
        }
        Defect::TagKeyTooLong => {
            let k = "k".repeat(too_long);
            rule.scope = one_tag(&k, "v");
            let (len, max) = (too_long, MAX_TAG_LENGTH);
            (format!("/scope/{k}"), Reason::TagKeyLength { len, max })
        }
        Defect::TagValueTooLong => {
            // The key holds `/` and `~`, the two characters that a JSON
            // pointer escapes. RFC 6901 writes `~` as `~0` and `/` as `~1`,
            // which gives the pointer expected below. This defect carries
            // that key, because the defect lies in the value, so the key can
            // hold any characters.
            rule.scope = one_tag("a/b~c", &"v".repeat(too_long));
            let (len, max) = (too_long, MAX_TAG_LENGTH);
            (
                "/scope/a~1b~0c".to_owned(),
                Reason::TagValueLength { len, max },
            )
        }
    }
}

/// The position, in the JSON form of a rule, of the field that `d` breaks.
fn field(d: Defect) -> usize {
    match d {
        Defect::NilRuleId | Defect::RandomRuleId => 0,
        Defect::SampleRateZero | Defect::SampleRateAboveOne | Defect::SampleRateNan => 1,
        Defect::TooManyTags
        | Defect::TagKeyEmpty
        | Defect::TagKeyTooLong
        | Defect::TagValueTooLong => 2,
    }
}

proptest! {
    // Without a regression file a failure leaves no untracked file in the
    // working tree. A failure prints the minimal failing input.
    #![proptest_config(ProptestConfig {
        failure_persistence: None,
        .. ProptestConfig::default()
    })]

    /// A valid rule writes to JSON, reads back equal, and validates.
    #[test]
    fn a_valid_rule_round_trips_and_validates(rule in rule()) {
        let json = serde_json::to_string(&rule).expect("a rule serializes");
        let back: Rule = serde_json::from_str(&json).expect("the JSON parses");
        // The floats in the rule compare exactly after the round trip.
        // serde_json prints the shortest text that names each f64, and its
        // float_roundtrip feature parses that text back to the same f64.
        prop_assert_eq!(&back, &rule);
        prop_assert_eq!(validate(&rule), Ok(()));
    }

    /// A rule whose JSON holds a property that the model does not have is
    /// refused.
    #[test]
    fn a_rule_with_an_unknown_property_is_refused(rule in rule()) {
        let mut document = serde_json::to_value(&rule).expect("a rule serializes");
        // The test copies the expression under the misspelled key
        // `expresion` and keeps the original, so the parse can fail only on
        // the unknown property. Renaming the key would also fail, but on the
        // missing `expression`, which has no default.
        document["expresion"] = document["expression"].clone();
        prop_assert!(serde_json::from_value::<Rule>(document).is_err());
    }

    /// A rule with one defect is refused with that defect's reason, at the
    /// JSON pointer of the defective field.
    #[test]
    fn a_defect_is_refused_at_its_place(mut rule in rule(), d in select(DEFECTS.to_vec())) {
        let (at, reason) = apply(d, &mut rule);
        let error = validate(&rule).expect_err("a defective rule is refused");
        // The test compares the refusal as a JSON value, because `Reason`
        // holds a NaN sample rate for one defect and a NaN is not equal to
        // itself. serde_json writes a NaN as null on both sides.
        prop_assert_eq!(
            serde_json::to_value(&error).expect("the error serializes"),
            serde_json::to_value(Error { at, reason }).expect("the error serializes"),
            "{:?}", d
        );
    }

    /// A rule with defects in two fields is refused for the field that the
    /// JSON form writes first.
    #[test]
    fn the_earlier_field_is_refused_first(
        mut rule in rule(),
        (a, b) in (select(DEFECTS.to_vec()), select(DEFECTS.to_vec())),
    ) {
        // Two defects of one field would overwrite each other, because each
        // defect replaces its field whole.
        prop_assume!(field(a) != field(b));
        // Each call changes its own field alone, so the order of the calls
        // does not count.
        let (at_a, reason_a) = apply(a, &mut rule);
        let (at_b, reason_b) = apply(b, &mut rule);
        let (at, reason) = if field(a) < field(b) {
            (at_a, reason_a)
        } else {
            (at_b, reason_b)
        };
        let error = validate(&rule).expect_err("a defective rule is refused");
        // As in the property above, the refusal is compared as a JSON value
        // because of the NaN sample rate.
        prop_assert_eq!(
            serde_json::to_value(&error).expect("the error serializes"),
            serde_json::to_value(Error { at, reason }).expect("the error serializes"),
            "{:?} and {:?}", a, b
        );
    }
}
