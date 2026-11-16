//! The limits that validation and the parser hold a rule to. They are
//! constants that no caller can set, so a rule is valid or invalid in the same
//! way on every install (ADR-0033).

/// Scope tags on a rule, at most (ADR-0034). Tags are dimensions of a
/// deployment. The cap bounds the tag cardinality of one rule.
pub const MAX_SCOPE_TAGS: usize = 8;

/// Characters in a tag key and in a tag value, at most. The least is 1
/// (ADR-0034).
pub const MAX_TAG_LENGTH: usize = 64;

/// Characters in a rule's text, at most (ADR-0042). The cap on length bounds
/// the parser's work only while no two alternatives of a choice read the same
/// bracketed text, and `MAX_PARSE_STEPS` refuses a text when they do.
pub const MAX_TEXT_LENGTH: usize = 4096;

/// Levels of nesting in a rule's text, at most (ADR-0042). The parser calls
/// itself once per level, so the cap bounds its stack.
pub const MAX_NESTING: usize = 32;

/// Token tests while parsing one text, at most (ADR-0042). The value is a
/// choice that no measurement backs, and the system holds a rule to limits on
/// purpose. The cap is reached only when two alternatives of the grammar read
/// the same bracketed text, and then the rule is refused and the parser does
/// not hang.
pub const MAX_PARSE_STEPS: usize = 1_000_000;
