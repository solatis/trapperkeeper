# engine/

**What**: the rule engine crate, `trapperkeeper-engine`, with the library
`trapperkeeper_engine`, which a pipeline can embed. It holds the rule model,
the tree of a rule's expression, their JSON form and the validation of a
rule's own fields. It also parses a rule's text into the tree. It does not
check an expression or evaluate rules yet. It depends on no server, store or HTTP code.

**Map**:

| File                  | What                                                                                   | When to read                                                                                  |
| --------------------- | -------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| `Cargo.toml`          | the package name; the dependencies and their features                                  | changing a dependency; why floats parse exactly; why no UUID generator                        |
| `src/lib.rs`          | the modules and what the crate exports                                                 | what a user of the crate can reach                                                            |
| `src/expr.rs`         | the expression tree, a path segment and the type names                                 | the JSON key of a kind of node; adding a function or a method                                 |
| `src/rule.rs`         | the rule and the action types                                                          | the JSON form of a rule; a default                                                            |
| `src/limits.rs`       | the limits as constants, each with where its number came from                          | how many scope tags, how long a tag; how long a rule's text may be and how deep it may nest   |
| `src/validate.rs`     | `validate`, the refusal reasons and the JSON pointer of a refusal                      | which fields of a rule are checked; why a rule was refused                                    |
| `src/text/`           | the parser of rule text                                                                | see `src/text/AGENTS.md`                                                                      |
| `tests/common/mod.rs` | the strategies for trees and for rules                                                 | generating trees or rules in another test; adding a kind of node to the generated trees       |
| `tests/tree.rs`       | the round trip and the refusals of the tree's JSON, as properties over generated trees | how the JSON form of the tree is tested                                                       |
| `tests/text.rs`       | valid and faulty rule texts, with the place, the reason and the sentence of each error | what an error says for a mistake; adding a faulty text                                        |
| `tests/model.rs`      | the defects, and the round-trip and refusal properties of a rule                       | how the JSON form of a rule and validation are tested; adding a refusal reason and its defect |

**Invariants**:

- Every later consumer assumes a rule that `validate` accepted. Nothing
  evaluates a rule that was not validated. `validate` does not check the
  expression yet.
- The limits are constants. A commit changes one, and no setting does.
- The crate depends on serde, serde_json and uuid only. A dependency on the
  server, the store or HTTP breaks the milestone goal that a pipeline can
  embed the crate.
- Every struct of the model and every node of the tree refuses an unknown
  JSON property.
- A new kind of node is added to the strategy `expr` in
  `tests/common/mod.rs`, so the properties of the tree cover it.
- The engine never generates a `rule_id`.
- A new refusal reason gets a defect in `tests/model.rs`, and a valid rule
  with that one defect gets that one refusal.

**Why**:

- The rule format and the form of the tree: `doc/adr/0033-rule-format.md`.
- The rule language, its kinds of expression and the closed list of functions
  and methods: `doc/adr/0041-the-rule-language.md`.
- The scope tags, their caps and the seen-scope index:
  `doc/adr/0034-scope-tags-and-the-seen-scope-index.md`.
- Properties are the preferred tests:
  `doc/adr/0018-unit-integration-and-end-to-end-tests.md`.
- The crate's directory: `doc/adr/0010-one-top-level-directory-per-component.md`.
  The reason for the package name sits at `[package]` in `Cargo.toml`.

**How to**: build, lint and test with the commands in the root `AGENTS.md`,
How to. `cargo test -p trapperkeeper-engine` runs this crate alone.
