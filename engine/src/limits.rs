//! The limits validation holds a rule to. They are part of the evaluation
//! contract every SDK shares, so they are constants and no caller can set
//! them (doc/adr/0033-rule-format.md).

/// Segments in a field path, at most (owner, 2026-11-10).
pub const MAX_PATH_DEPTH: usize = 8;

/// Wildcards in one field path, at most (owner, 2026-11-10). With one
/// wildcard a condition visits one list once, so its cost is bounded by the
/// depth plus that list's length.
pub const MAX_WILDCARDS_PER_PATH: usize = 1;

/// Values in a membership list, at most (owner, 2026-11-10).
pub const MAX_LIST_VALUES: usize = 32;

/// Conditions in a rule across all its groups, at most. The groups are not
/// counted (owner, 2026-11-10). At 64, a rule is a bounded amount of work per
/// record and is still readable in a rule builder.
pub const MAX_CONDITIONS: usize = 64;

/// Scope tags on a rule, at most. Tags are dimensions of a deployment, and
/// the seen-scope index stays finite only while they are few (doc/adr/0034).
pub const MAX_SCOPE_TAGS: usize = 8;

/// Characters in a tag key and in a tag value, at most. The least is 1
/// (doc/adr/0034).
pub const MAX_TAG_LENGTH: usize = 64;
