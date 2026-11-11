# engine/

**What**: the rule engine crate, `trapperkeeper-engine`, with the library
`trapperkeeper_engine`, which a pipeline can embed. It holds the rule model,
the model's JSON form and the validation of a rule. It does not evaluate
rules yet. It depends on no server, store or HTTP code.

**Map**:

| File              | What                                                                                   | When to read                                                                                 |
| ----------------- | -------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| `Cargo.toml`      | the package name; the dependencies and their features                                  | changing a dependency; why floats parse exactly; why no UUID generator                       |
| `src/lib.rs`      | the modules and what the crate exports                                                 | what a user of the crate can reach                                                           |
| `src/path.rs`     | the field path and its segments                                                        | how a condition names a field; the JSON of a key, an index and the wildcard                  |
| `src/rule.rs`     | the rule, group, condition, value, operator, policy, field type and action types       | the JSON form of a rule; a default; adding a field or an operator                            |
| `src/limits.rs`   | the limits as constants, each with where its number came from                          | how deep a path, how long a list, how many conditions or scope tags                          |
| `src/validate.rs` | `validate`, the refusal reasons and the JSON pointer of a refusal                      | which operator fits which type; why a rule was refused; adding a shape rule                  |
| `tests/format.rs` | one literal rule document that pins the names and the order of the JSON form           | what a rule looks like in JSON; a renamed property                                           |
| `tests/model.rs`  | the strategies for valid rules, the defects, and the round-trip and refusal properties | how validation is tested; adding a refusal reason and its defect; generating rules elsewhere |

**Invariants**:

- Every later consumer assumes a rule that `validate` accepted. Nothing
  evaluates a rule that was not validated.
- The limits are constants. A commit changes one, and no setting does (owner,
  2026-11-10).
- The crate depends on serde, serde_json and uuid only. A dependency on the
  server, the store or HTTP breaks the milestone goal that a pipeline can
  embed the crate.
- Every struct of the model refuses an unknown JSON property.
- The engine never generates a `rule_id`.
- A new refusal reason gets a defect in `tests/model.rs`, and a valid rule
  with that one defect gets that one refusal.

**Why**:

- The rule format, with each owner decision and its date:
  `doc/adr/0033-rule-format.md`.
- The scope tags, their caps and the seen-scope index:
  `doc/adr/0034-scope-tags-and-the-seen-scope-index.md`.
- One vocabulary for failure behavior, which gives the two policies their
  words: `doc/adr/0005-one-vocabulary-for-failure-behavior.md`.
- The field types: `doc/adr/0006-conformance-fixtures-are-the-evaluation-spec.md`.
- Properties are the preferred tests:
  `doc/adr/0018-unit-integration-and-end-to-end-tests.md`.
- The crate's directory: `doc/adr/0010-one-top-level-directory-per-component.md`.
  The reason for the package name sits at `[package]` in `Cargo.toml`.

**How to**: build, lint and test with the commands in the root `AGENTS.md`,
How to. `cargo test -p trapperkeeper-engine` runs this crate alone.
