//! The limits that validation and the parser hold a rule to. They are part
//! of the contract every SDK shares, so they are constants and no caller can
//! set them (ADR-0033).

/// Scope tags on a rule, at most. Tags are dimensions of a deployment, and
/// the seen-scope index stays finite only while they are few (ADR-0034).
pub const MAX_SCOPE_TAGS: usize = 8;

/// Characters in a tag key and in a tag value, at most. The least is 1
/// (ADR-0034).
pub const MAX_TAG_LENGTH: usize = 64;

/// Characters in a rule's text, at most (ADR-0042). The parser reads a text
/// once, so the cap bounds the work that one rule can ask of it.
pub const MAX_TEXT_LENGTH: usize = 4096;

/// Levels of nesting in a rule's text, at most (ADR-0042). The parser calls
/// itself once per level, so the cap bounds its stack.
pub const MAX_NESTING: usize = 32;

/// Token tests while parsing one text, at most. A text of the greatest
/// length parses in far fewer. The cap is reached only when two alternatives
/// of the grammar read the same bracketed text, and then the rule is refused
/// and the parser does not hang.
pub const MAX_PARSE_STEPS: usize = 1_000_000;
