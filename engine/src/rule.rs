//! The rule: its identity, its action, its sample rate, its scope and its one
//! expression, with their JSON form. ADR-0033 decides the format of a rule
//! and gives the reasons behind it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::expr::Expr;

/// What a sensor does with a record that matched. `Observe` only reports the
/// match, `Drop` removes the record from the pipeline, and `Throw` raises an
/// error into the pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Observe,
    Drop,
    Throw,
}

/// A rule: one expression with its identity, action, sample rate and scope.
/// ADR-0033 makes a rule immutable, so an edit is a new rule with a new id.
/// The type does not enforce that.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
// A JSON property that the rule does not know, such as a misspelled one, fails
// the parse and does not silently take its default.
#[serde(deny_unknown_fields)]
pub struct Rule {
    // The server generates the id and the engine reads it. Validation holds
    // it to a version 7 UUID, the time-ordered kind of ADR-0020.
    pub rule_id: Uuid,
    // A display name in the `_title` form of ADR-0020. It is not part of
    // the identity and may repeat.
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
    // The tree that a planner reads (ADR-0033). The rule's text is not kept,
    // because it can be written from the tree.
    pub expression: Expr,
}

fn every_record() -> f64 {
    1.0
}
