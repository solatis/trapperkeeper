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
/// `MAX_PARSE_STEPS` refuses the text.
pub const MAX_TEXT_LENGTH: usize = 4096;

/// Levels of nesting in a rule's text, at most (ADR-0042). A bracket, a
/// `not`, a negating minus and the branch after `else` each open a level. The
/// parser calls itself once per level, so the cap bounds its stack. The cap
/// does not bound the depth of a tree, because a chain of operators such as
/// `a + b + c` nests without a bracket. `MAX_DEPTH` bounds that.
pub const MAX_NESTING: usize = 32;

/// Nodes on the way from the top of a rule's expression down to a leaf, at
/// most, with the top and the leaf counted (ADR-0033). A node takes up to two
/// levels of JSON, and the JSON reader stops at 128 levels, so a tree within
/// the cap reads back from its JSON. Code that walks a tree calls itself once
/// per level, so the cap also bounds its stack.
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
