//! A field path: how a condition names a field in a record.

use serde::{Deserialize, Serialize};

/// One step of a field path: a key into an object, an index into a list, or
/// a wildcard that stands for every element of a list. In JSON a key is
/// `{"key": "order"}`, an index is `{"index": 0}` and the wildcard is `"*"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Segment {
    Key(String),
    Index(u32),
    // serde writes a unit variant as a bare string, and the rename makes that
    // string `*`.
    #[serde(rename = "*")]
    Wildcard,
}

/// A field path: the segments in order, resolved against a record at
/// evaluation time. Validation bounds its length and its wildcards.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FieldPath(pub Vec<Segment>);

impl FieldPath {
    /// How many segments are wildcards.
    #[must_use]
    pub fn wildcards(&self) -> usize {
        self.0
            .iter()
            .filter(|s| matches!(s, Segment::Wildcard))
            .count()
    }
}
