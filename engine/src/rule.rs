//! The rule types and their JSON form. A rule is an OR of groups, and each
//! group is an AND of conditions. The format and the decisions behind it are
//! ADR-0033. Every struct refuses a JSON property it does
//! not know, so a misspelled property is a parse error and never a default.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::path::FieldPath;

/// What a sensor does with a record that matched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Observe,
    Drop,
    Fail,
}

/// The type a condition declares for its field. The field's value is coerced
/// to it at evaluation (ADR-0006).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    Int,
    Float,
    String,
    Boolean,
    Any,
}

/// How a condition compares the field with its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operator {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    In,
    NotIn,
    Contains,
    StartsWith,
    EndsWith,
}

/// What a condition does when its field is missing or cannot be coerced:
/// `skip` does not match, `match` matches, `fail` fails the rule
/// (ADR-0005).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Policy {
    // Skip is the default, because every default degrades toward
    // pass-through (doc/principles.md, 1).
    #[default]
    Skip,
    Match,
    Fail,
}

/// One value a condition compares with: a JSON scalar, typed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Scalar {
    // Int comes before Float because serde tries untagged variants in order.
    // An integer literal becomes Int, and only a literal with a fraction or
    // an exponent becomes Float.
    Int(i64),
    Float(f64),
    String(String),
    Boolean(bool),
}

/// A condition's value: one scalar, or a list for the membership operators.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    One(Scalar),
    Many(Vec<Scalar>),
}

/// One test on one field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Condition {
    pub path: FieldPath,
    pub field_type: FieldType,
    pub op: Operator,
    pub value: Value,
    // A policy that is absent in JSON is `skip` (ADR-0005).
    #[serde(default)]
    pub on_missing: Policy,
    #[serde(default)]
    pub on_coercion_failure: Policy,
}

/// An AND of conditions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub conditions: Vec<Condition>,
}

/// A rule: an OR of groups with its identity, action, sample rate and scope.
/// A rule is immutable, and an edit is a new rule with a new id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    // The server generates the id and the engine reads it. Validation holds
    // it to version 7, the time-ordered kind of ADR-0020.
    pub rule_id: Uuid,
    // A display name in the `_title` form of ADR-0020. It is not part of
    // the identity and may repeat. JSON omits it when it is absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_title: Option<String>,
    pub action: Action,
    // The fraction of the records the rule is evaluated on, 1 when absent in
    // JSON. Validation holds it to (0, 1].
    #[serde(default = "every_record")]
    pub sample_rate: f64,
    // The key-value tags a sensor matches its own tags against
    // (ADR-0034). The map is ordered, so the JSON is the same bytes every
    // time.
    #[serde(default)]
    pub scope: BTreeMap<String, String>,
    pub groups: Vec<Group>,
}

/// The default sample rate: every record.
fn every_record() -> f64 {
    1.0
}
