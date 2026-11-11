//! Strict validation of a rule, before it is stored or sent to a sensor.
//! The evaluator assumes a rule that passed.

use serde::Serialize;

use crate::limits::{
    MAX_CONDITIONS, MAX_LIST_VALUES, MAX_PATH_DEPTH, MAX_SCOPE_TAGS, MAX_TAG_LENGTH,
    MAX_WILDCARDS_PER_PATH,
};
use crate::path::{FieldPath, Segment};
use crate::rule::{Condition, FieldType, Operator, Rule, Scalar, Value};

/// Why a part of a rule was refused.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "reason")]
pub enum Reason {
    NoGroups,
    EmptyGroup,
    TooManyConditions {
        count: usize,
        max: usize,
    },
    EmptyPath,
    PathTooDeep {
        depth: usize,
        max: usize,
    },
    TooManyWildcards {
        count: usize,
        max: usize,
    },
    EmptyKey,
    EmptyList,
    ListTooLong {
        count: usize,
        max: usize,
    },
    OperatorNeedsOneValue,
    OperatorNeedsList,
    OperatorNotForType {
        op: Operator,
        field_type: FieldType,
    },
    ValueNotOfType {
        expected: FieldType,
        found: &'static str,
    },
    NonFiniteFloat,
    SampleRateOutOfRange {
        sample_rate: f64,
    },
    RuleIdNotTimeOrdered {
        version: usize,
    },
    TooManyTags {
        count: usize,
        max: usize,
    },
    TagKeyLength {
        len: usize,
        max: usize,
    },
    TagValueLength {
        len: usize,
        max: usize,
    },
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

/// The name of a scalar's type for a refusal: `int`, `float`, `string` or
/// `boolean`.
fn type_name(s: &Scalar) -> &'static str {
    match s {
        Scalar::Int(_) => "int",
        Scalar::Float(_) => "float",
        Scalar::String(_) => "string",
        Scalar::Boolean(_) => "boolean",
    }
}

/// Whether `s` is of `t`, with an int accepted as a float and any scalar
/// accepted as `any`.
fn is_of_type(s: &Scalar, t: FieldType) -> bool {
    matches!(
        (s, t),
        (_, FieldType::Any)
            | (Scalar::Int(_), FieldType::Int | FieldType::Float)
            | (Scalar::Float(_), FieldType::Float)
            | (Scalar::String(_), FieldType::String)
            | (Scalar::Boolean(_), FieldType::Boolean)
    )
}

/// Adds the refusals of one field path at `at` to `errors`.
fn check_path(at: &str, path: &FieldPath, errors: &mut Vec<Error>) {
    let mut refuse = |at: String, reason| errors.push(Error { at, reason });

    let depth = path.0.len();
    if depth == 0 {
        refuse(at.to_owned(), Reason::EmptyPath);
    }
    if depth > MAX_PATH_DEPTH {
        let max = MAX_PATH_DEPTH;
        refuse(at.to_owned(), Reason::PathTooDeep { depth, max });
    }
    let count = path.wildcards();
    if count > MAX_WILDCARDS_PER_PATH {
        let max = MAX_WILDCARDS_PER_PATH;
        refuse(at.to_owned(), Reason::TooManyWildcards { count, max });
    }
    for (i, segment) in path.0.iter().enumerate() {
        // A key of zero length addresses nothing in any record.
        if matches!(segment, Segment::Key(k) if k.is_empty()) {
            refuse(format!("{at}/{i}/key"), Reason::EmptyKey);
        }
    }
}

/// Adds the refusals of one condition's operator, type and value to
/// `errors`. `at` points at the condition. A condition gets the refusals of
/// one check only, the first that fails.
fn check_condition(at: &str, c: &Condition, errors: &mut Vec<Error>) {
    // 1. the operator is allowed on the declared type
    // 2. the value's arity fits the operator: one scalar, or a list of 1 to
    //    MAX_LIST_VALUES
    // 3. each scalar is of the declared type, and a float is finite
    let mut refuse = |at: String, reason| errors.push(Error { at, reason });

    // 1. The ordering operators need an order, which a boolean does not have
    //    and an `any` field does not have before coercion. The string tests
    //    need a string, which only `string` declares.
    let (op, field_type) = (c.op, c.field_type);
    let allowed = match op {
        Operator::Eq | Operator::Ne | Operator::In | Operator::NotIn => true,
        Operator::Lt | Operator::Le | Operator::Gt | Operator::Ge => matches!(
            field_type,
            FieldType::Int | FieldType::Float | FieldType::String
        ),
        Operator::Contains | Operator::StartsWith | Operator::EndsWith => {
            field_type == FieldType::String
        }
    };
    if !allowed {
        refuse(
            format!("{at}/op"),
            Reason::OperatorNotForType { op, field_type },
        );
        return;
    }

    // 2. Only the membership operators take a list.
    let membership = matches!(op, Operator::In | Operator::NotIn);
    let value = format!("{at}/value");
    let scalars: Vec<(String, &Scalar)> = match (&c.value, membership) {
        (Value::One(s), false) => vec![(value, s)],
        (Value::Many(list), true) => {
            let count = list.len();
            if count == 0 {
                refuse(value, Reason::EmptyList);
                return;
            }
            if count > MAX_LIST_VALUES {
                let max = MAX_LIST_VALUES;
                refuse(value, Reason::ListTooLong { count, max });
                return;
            }
            let at = |(i, s)| (format!("{value}/{i}"), s);
            list.iter().enumerate().map(at).collect()
        }
        (Value::One(_), true) => {
            refuse(value, Reason::OperatorNeedsList);
            return;
        }
        (Value::Many(_), false) => {
            refuse(value, Reason::OperatorNeedsOneValue);
            return;
        }
    };

    // 3. An int is accepted where a float is declared, because every
    //    language widens it. A float is refused where an int is declared,
    //    because a fraction there is more likely a mistake.
    for (at, s) in scalars {
        if !is_of_type(s, field_type) {
            let (expected, found) = (field_type, type_name(s));
            refuse(at, Reason::ValueNotOfType { expected, found });
        } else if matches!(s, Scalar::Float(f) if !f.is_finite()) {
            // JSON cannot carry a NaN or an infinity. serde_json writes
            // either as null, which no scalar reads back.
            refuse(at, Reason::NonFiniteFloat);
        }
    }
}

/// Checks `rule` against the shape rules and the limits and returns every
/// refusal, so a caller fixes them in one pass.
///
/// # Errors
///
/// The refusals, in document order, when there is at least one.
pub fn validate(rule: &Rule) -> Result<(), Vec<Error>> {
    // 1. the rule's own fields: the id's UUID version, the sample rate, the
    //    tags against MAX_SCOPE_TAGS and MAX_TAG_LENGTH, at least one group
    // 2. the total number of conditions against MAX_CONDITIONS
    // 3. each group: not empty
    // 4. each condition: its path, then its operator, type and value
    let mut errors = Vec::new();
    let mut refuse = |at: String, reason| errors.push(Error { at, reason });

    // 1. The id must be version 7, the time-ordered kind the server issues
    //    (doc/adr/0020). The nil id is version 0 and fails the same check.
    let version = rule.rule_id.get_version_num();
    if version != 7 {
        refuse(
            "/rule_id".to_owned(),
            Reason::RuleIdNotTimeOrdered { version },
        );
    }
    // 1. A rate of 0 is refused because a rule that samples nothing is a
    //    mistake. A NaN fails both comparisons, so it is refused too.
    let sample_rate = rule.sample_rate;
    if !(sample_rate > 0.0 && sample_rate <= 1.0) {
        refuse(
            "/sample_rate".to_owned(),
            Reason::SampleRateOutOfRange { sample_rate },
        );
    }
    // 1. The caps keep the seen-scope index finite (doc/adr/0034). A key or
    //    a value of zero length addresses nothing on any sensor.
    let count = rule.scope.len();
    if count > MAX_SCOPE_TAGS {
        let max = MAX_SCOPE_TAGS;
        refuse("/scope".to_owned(), Reason::TooManyTags { count, max });
    }
    let max = MAX_TAG_LENGTH;
    for (key, value) in &rule.scope {
        // Both refusals point at the tag, because a JSON pointer cannot
        // point at a key.
        let at = format!("/scope/{}", token(key));
        let len = key.chars().count();
        if !(1..=max).contains(&len) {
            refuse(at.clone(), Reason::TagKeyLength { len, max });
        }
        let len = value.chars().count();
        if !(1..=max).contains(&len) {
            refuse(at, Reason::TagValueLength { len, max });
        }
    }
    if rule.groups.is_empty() {
        refuse("/groups".to_owned(), Reason::NoGroups);
    }

    // 2. The cap is one number for the rule, so the count runs across the
    //    groups.
    let count = rule.groups.iter().map(|g| g.conditions.len()).sum();
    if count > MAX_CONDITIONS {
        let max = MAX_CONDITIONS;
        refuse(
            "/groups".to_owned(),
            Reason::TooManyConditions { count, max },
        );
    }

    for (i, group) in rule.groups.iter().enumerate() {
        if group.conditions.is_empty() {
            errors.push(Error {
                at: format!("/groups/{i}/conditions"),
                reason: Reason::EmptyGroup,
            });
        }
        // 4. The pointers follow the field names of the JSON form, so a
        //    server can hand them to a client as they are.
        for (j, c) in group.conditions.iter().enumerate() {
            let at = format!("/groups/{i}/conditions/{j}");
            check_path(&format!("{at}/path"), &c.path, &mut errors);
            check_condition(&at, c, &mut errors);
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
