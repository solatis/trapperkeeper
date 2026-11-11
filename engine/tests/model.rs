//! Properties of the rule model: a generated valid rule round-trips through
//! JSON unchanged and passes validation, and one defect applied to it is
//! refused with that defect's reason at its place. Each claim is one property
//! over generated rules, as ADR-0018 asks.

use std::collections::BTreeMap;

use proptest::collection::{btree_map, vec};
use proptest::prelude::*;
use proptest::sample::select;
use trapperkeeper_engine::limits::{
    MAX_CONDITIONS, MAX_LIST_VALUES, MAX_PATH_DEPTH, MAX_SCOPE_TAGS, MAX_TAG_LENGTH,
};
use trapperkeeper_engine::{
    Action, Condition, Error, FieldPath, FieldType, Group, Operator, Policy, Reason, Rule, Scalar,
    Segment, Value, validate,
};
use uuid::Uuid;

/// A scalar that validation accepts for `t`, finite when it is a float.
fn scalar(t: FieldType) -> BoxedStrategy<Scalar> {
    let int = any::<i64>().prop_map(Scalar::Int);
    let float = {
        use proptest::num::f64::{NEGATIVE, NORMAL, POSITIVE, SUBNORMAL, ZERO};
        (POSITIVE | NEGATIVE | NORMAL | SUBNORMAL | ZERO).prop_map(Scalar::Float)
    };
    let string = any::<String>().prop_map(Scalar::String);
    let boolean = any::<bool>().prop_map(Scalar::Boolean);
    match t {
        FieldType::Int => int.boxed(),
        // A float field also takes an int value.
        FieldType::Float => prop_oneof![float, int].boxed(),
        FieldType::String => string.boxed(),
        FieldType::Boolean => boolean.boxed(),
        FieldType::Any => prop_oneof![int, float, string, boolean].boxed(),
    }
}

/// A path within the limits: 1 to `MAX_PATH_DEPTH` segments, at most one
/// wildcard, keys non-empty.
fn path() -> impl Strategy<Value = FieldPath> {
    let segment = prop_oneof![
        ".{1,12}".prop_map(Segment::Key),
        any::<u32>().prop_map(Segment::Index),
    ];
    let wildcard_at = proptest::option::of(any::<prop::sample::Index>());
    (vec(segment, 1..=MAX_PATH_DEPTH), wildcard_at).prop_map(|(mut segments, wildcard_at)| {
        if let Some(i) = wildcard_at {
            let i = i.index(segments.len());
            segments[i] = Segment::Wildcard;
        }
        FieldPath(segments)
    })
}

/// The operators validation allows on `t`.
fn operators(t: FieldType) -> Vec<Operator> {
    use Operator::{Contains, EndsWith, Eq, Ge, Gt, In, Le, Lt, Ne, NotIn, StartsWith};
    let mut ops = vec![Eq, Ne, In, NotIn];
    if matches!(t, FieldType::Int | FieldType::Float | FieldType::String) {
        ops.extend([Lt, Le, Gt, Ge]);
    }
    if t == FieldType::String {
        ops.extend([Contains, StartsWith, EndsWith]);
    }
    ops
}

fn policy() -> impl Strategy<Value = Policy> {
    select(vec![Policy::Skip, Policy::Match, Policy::Fail])
}

/// A condition whose operator fits its type and whose value fits both.
fn condition() -> impl Strategy<Value = Condition> {
    use FieldType::{Any, Boolean, Float, Int, String};
    let typed = select(vec![Int, Float, String, Boolean, Any])
        .prop_flat_map(|t| (Just(t), select(operators(t))))
        .prop_flat_map(|(t, op)| {
            let value = if matches!(op, Operator::In | Operator::NotIn) {
                // For `any`, the list mixes scalars of every type.
                vec(scalar(t), 1..=MAX_LIST_VALUES)
                    .prop_map(Value::Many)
                    .boxed()
            } else {
                scalar(t).prop_map(Value::One).boxed()
            };
            (Just(t), Just(op), value)
        });
    (path(), typed, policy(), policy()).prop_map(
        |(path, (field_type, op, value), on_missing, on_coercion_failure)| Condition {
            path,
            field_type,
            op,
            value,
            on_missing,
            on_coercion_failure,
        },
    )
}

/// A version 7 id from a generated timestamp and generated random bytes.
fn rule_id() -> impl Strategy<Value = Uuid> {
    // A version 7 id holds 48 bits of milliseconds.
    (0..1u64 << 48, any::<[u8; 10]>())
        .prop_map(|(millis, bytes)| uuid::Builder::from_unix_timestamp_millis(millis, &bytes))
        .prop_map(uuid::Builder::into_uuid)
}

/// A scope within the caps: 0 to `MAX_SCOPE_TAGS` tags, each key and value
/// of 1 to `MAX_TAG_LENGTH` characters.
fn scope() -> impl Strategy<Value = BTreeMap<String, String>> {
    let text = || {
        proptest::string::string_regex(&format!(".{{1,{MAX_TAG_LENGTH}}}"))
            .expect("a valid pattern")
    };
    btree_map(text(), text(), 0..=MAX_SCOPE_TAGS)
}

/// A rule within every limit: a version 7 id, a sample rate in (0, 1], a
/// scope within the caps, and 1 to 8 groups of 1 to 8 conditions each.
fn rule() -> impl Strategy<Value = Rule> {
    let title = proptest::option::of(".{0,40}");
    let action = select(vec![Action::Observe, Action::Drop, Action::Fail]);
    let sample_rate = prop_oneof![Just(1.0), f64::MIN_POSITIVE..=1.0];
    // 8 groups of 8 conditions is MAX_CONDITIONS, so every generated rule
    // stays within the cap.
    let groups = vec(
        vec(condition(), 1..=8).prop_map(|conditions| Group { conditions }),
        1..=8,
    );
    (rule_id(), title, action, sample_rate, scope(), groups).prop_map(
        |(rule_id, rule_title, action, sample_rate, scope, groups)| Rule {
            rule_id,
            rule_title,
            action,
            sample_rate,
            scope,
            groups,
        },
    )
}

/// One way to break a valid rule.
#[derive(Debug, Clone, Copy)]
enum Defect {
    NoGroups,
    EmptyGroup,
    TooManyConditions,
    EmptyPath,
    PathTooDeep,
    TwoWildcards,
    EmptyKey,
    EmptyList,
    ListTooLong,
    ListOnEq,
    ScalarOnIn,
    OrderOnBoolean,
    StringTestOnInt,
    FloatOnInt,
    StringOnInt,
    NanValue,
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

const DEFECTS: [Defect; 25] = [
    Defect::NoGroups,
    Defect::EmptyGroup,
    Defect::TooManyConditions,
    Defect::EmptyPath,
    Defect::PathTooDeep,
    Defect::TwoWildcards,
    Defect::EmptyKey,
    Defect::EmptyList,
    Defect::ListTooLong,
    Defect::ListOnEq,
    Defect::ScalarOnIn,
    Defect::OrderOnBoolean,
    Defect::StringTestOnInt,
    Defect::FloatOnInt,
    Defect::StringOnInt,
    Defect::NanValue,
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

/// A valid condition on the int field `a`, with `op` and `value`.
fn on_int(op: Operator, value: Value) -> Condition {
    Condition {
        path: FieldPath(vec![Segment::Key("a".to_owned())]),
        field_type: FieldType::Int,
        op,
        value,
        on_missing: Policy::Skip,
        on_coercion_failure: Policy::Skip,
    }
}

/// The scope with one tag.
fn one_tag(key: &str, value: &str) -> BTreeMap<String, String> {
    BTreeMap::from([(key.to_owned(), value.to_owned())])
}

/// Applies `d` to `rule` and returns the pointer and the reason `validate`
/// must report. A defect of a condition replaces the first condition of the
/// first group, and a defect of a tag replaces the scope.
#[expect(
    clippy::too_many_lines,
    reason = "one arm per defect, and the arms share nothing"
)]
fn apply(d: Defect, rule: &mut Rule) -> (String, Reason) {
    let one = |s| Value::One(s);
    let ints = |n: i64| Value::Many((0..n).map(Scalar::Int).collect());
    let key = |k: &str| Segment::Key(k.to_owned());
    let first = "/groups/0/conditions/0";
    let mut replace = |c: Condition| rule.groups[0].conditions[0] = c;
    let mut replace_path = |segments: Vec<Segment>| {
        let mut c = on_int(Operator::Eq, one(Scalar::Int(1)));
        c.path = FieldPath(segments);
        replace(c);
    };
    let too_long = MAX_TAG_LENGTH + 1;

    match d {
        Defect::NoGroups => {
            rule.groups.clear();
            ("/groups".to_owned(), Reason::NoGroups)
        }
        Defect::EmptyGroup => {
            rule.groups[0].conditions.clear();
            ("/groups/0/conditions".to_owned(), Reason::EmptyGroup)
        }
        Defect::TooManyConditions => {
            // The first group is filled with clones of its first condition,
            // which is valid, so the count is the only rule that breaks.
            let c = rule.groups[0].conditions[0].clone();
            rule.groups[0].conditions.resize(MAX_CONDITIONS + 1, c);
            let count = rule.groups.iter().map(|g| g.conditions.len()).sum();
            let max = MAX_CONDITIONS;
            (
                "/groups".to_owned(),
                Reason::TooManyConditions { count, max },
            )
        }
        Defect::EmptyPath => {
            replace_path(vec![]);
            (format!("{first}/path"), Reason::EmptyPath)
        }
        Defect::PathTooDeep => {
            let (depth, max) = (MAX_PATH_DEPTH + 1, MAX_PATH_DEPTH);
            replace_path(vec![key("a"); depth]);
            (format!("{first}/path"), Reason::PathTooDeep { depth, max })
        }
        Defect::TwoWildcards => {
            replace_path(vec![key("a"), Segment::Wildcard, Segment::Wildcard]);
            let (count, max) = (2, 1);
            (
                format!("{first}/path"),
                Reason::TooManyWildcards { count, max },
            )
        }
        Defect::EmptyKey => {
            replace_path(vec![key("a"), key("")]);
            (format!("{first}/path/1/key"), Reason::EmptyKey)
        }
        Defect::EmptyList => {
            replace(on_int(Operator::In, ints(0)));
            (format!("{first}/value"), Reason::EmptyList)
        }
        Defect::ListTooLong => {
            let (count, max) = (MAX_LIST_VALUES + 1, MAX_LIST_VALUES);
            let n = i64::try_from(count).expect("a small number");
            replace(on_int(Operator::NotIn, ints(n)));
            (format!("{first}/value"), Reason::ListTooLong { count, max })
        }
        Defect::ListOnEq => {
            replace(on_int(Operator::Eq, ints(1)));
            (format!("{first}/value"), Reason::OperatorNeedsOneValue)
        }
        Defect::ScalarOnIn => {
            replace(on_int(Operator::In, one(Scalar::Int(1))));
            (format!("{first}/value"), Reason::OperatorNeedsList)
        }
        Defect::OrderOnBoolean => {
            let (op, field_type) = (Operator::Lt, FieldType::Boolean);
            let mut c = on_int(op, one(Scalar::Boolean(true)));
            c.field_type = field_type;
            replace(c);
            (
                format!("{first}/op"),
                Reason::OperatorNotForType { op, field_type },
            )
        }
        Defect::StringTestOnInt => {
            let (op, field_type) = (Operator::Contains, FieldType::Int);
            replace(on_int(op, one(Scalar::Int(1))));
            (
                format!("{first}/op"),
                Reason::OperatorNotForType { op, field_type },
            )
        }
        Defect::FloatOnInt => {
            replace(on_int(Operator::Eq, one(Scalar::Float(1.5))));
            let (expected, found) = (FieldType::Int, "float");
            (
                format!("{first}/value"),
                Reason::ValueNotOfType { expected, found },
            )
        }
        Defect::StringOnInt => {
            // The second value of a list, so the pointer names the element.
            let list = vec![Scalar::Int(1), Scalar::String("1".to_owned())];
            replace(on_int(Operator::In, Value::Many(list)));
            let (expected, found) = (FieldType::Int, "string");
            (
                format!("{first}/value/1"),
                Reason::ValueNotOfType { expected, found },
            )
        }
        Defect::NanValue => {
            let mut c = on_int(Operator::Eq, one(Scalar::Float(f64::NAN)));
            c.field_type = FieldType::Float;
            replace(c);
            (format!("{first}/value"), Reason::NonFiniteFloat)
        }
        Defect::SampleRateZero | Defect::SampleRateAboveOne | Defect::SampleRateNan => {
            let sample_rate = match d {
                Defect::SampleRateZero => 0.0,
                Defect::SampleRateAboveOne => 1.0 + f64::EPSILON,
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
            let version = 0;
            (
                "/rule_id".to_owned(),
                Reason::RuleIdNotTimeOrdered { version },
            )
        }
        Defect::RandomRuleId => {
            // A well-formed version 4 id, so the refusal is about the
            // version alone.
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
            ("/scope/".to_owned(), Reason::TagKeyLength { len, max })
        }
        Defect::TagKeyTooLong => {
            let k = "k".repeat(too_long);
            rule.scope = one_tag(&k, "v");
            let (len, max) = (too_long, MAX_TAG_LENGTH);
            (format!("/scope/{k}"), Reason::TagKeyLength { len, max })
        }
        Defect::TagValueTooLong => {
            // The key holds the two characters a JSON pointer escapes.
            rule.scope = one_tag("a/b~c", &"v".repeat(too_long));
            let (len, max) = (too_long, MAX_TAG_LENGTH);
            (
                "/scope/a~1b~0c".to_owned(),
                Reason::TagValueLength { len, max },
            )
        }
    }
}

proptest! {
    // No regression file, so a failure leaves nothing untracked in the tree.
    // The failing seed is in the output.
    #![proptest_config(ProptestConfig {
        failure_persistence: None,
        .. ProptestConfig::default()
    })]

    /// A valid rule writes to JSON, reads back equal, and validates.
    #[test]
    fn a_valid_rule_round_trips_and_validates(rule in rule()) {
        let json = serde_json::to_string(&rule).expect("a rule serializes");
        let back: Rule = serde_json::from_str(&json).expect("the JSON parses");
        // The floats compare exactly: serde_json prints the shortest text
        // that names the f64, and its float_roundtrip feature parses that
        // text to the nearest f64.
        prop_assert_eq!(&back, &rule);
        prop_assert_eq!(validate(&rule), Ok(()));
    }

    /// One defect is refused with its reason at its place, and nothing else
    /// is reported.
    #[test]
    fn a_defect_is_refused_at_its_place(mut rule in rule(), d in select(DEFECTS.to_vec())) {
        let (at, reason) = apply(d, &mut rule);
        let errors = validate(&rule).expect_err("a defective rule is refused");
        // Compared as JSON, because a NaN sample rate inside a reason is not
        // equal to itself, and JSON writes it as null on both sides.
        prop_assert_eq!(
            serde_json::to_value(&errors).expect("the errors serialize"),
            serde_json::to_value([Error { at, reason }]).expect("the error serializes"),
            "{:?}", d
        );
    }
}
