//! The limits that validation and the parser hold a rule to. They are
//! constants that no caller can set, so a rule is valid or invalid in the same
//! way on every install (ADR-0033).

/// Scope tags on a rule, at most (ADR-0034). Tags are dimensions of a
/// deployment. The cap bounds the tag cardinality of one rule.
pub const MAX_SCOPE_TAGS: usize = 8;

/// Characters in a tag key and in a tag value, at most, counted as Unicode
/// scalar values. The least is 1 (ADR-0034).
pub const MAX_TAG_LENGTH: usize = 64;

/// Characters in a rule's text, at most, counted as Unicode scalar values
/// (ADR-0042). The cap on length bounds the parser's work only while no two
/// alternatives of a choice read the same bracketed text. Where two do, each
/// level of brackets is read twice, the work doubles per level, and
/// `MAX_PARSE_STEPS` refuses the text. Validation uses the same cap for
/// the printed text of an expression (ADR-0033).
pub const MAX_TEXT_LENGTH: usize = 4096;

/// Levels of nesting in a rule's text, at most (ADR-0042). A bracket, a
/// `not`, a negating minus and the branch after `else` each open a level. The
/// parser calls itself once per level, so the cap bounds its stack. This cap
/// does not limit how deep a tree gets: `a + b + c` nests without a bracket.
/// `MAX_DEPTH` limits that.
pub const MAX_NESTING: usize = 32;

/// How many levels deep an expression may be, where the top node is level 1
/// (ADR-0033). One node takes up to two levels of JSON and the JSON reader
/// stops at 128, so a tree within the cap can always be read back from its
/// JSON. The cap also keeps code that walks a tree from overflowing the
/// stack.
pub const MAX_DEPTH: usize = 32;

/// Token tests while parsing one text, at most (ADR-0042). A token test is
/// one comparison of a token with what a rule of the grammar wants at its
/// place. The value is a choice that no measurement backs, and the cap is
/// there because a system without limits is open to abuse (ADR-0033). The cap
/// is reached only when two alternatives of the grammar read the same
/// bracketed text, and then the rule is refused and the parser does not hang.
/// The grammar keeps the rule that no two alternatives do
/// (`engine/src/text/AGENTS.md`), so the cap guards against a defect of the
/// grammar.
pub const MAX_PARSE_STEPS: usize = 1_000_000;
