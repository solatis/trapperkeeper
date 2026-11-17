# engine/

**What**: the rule engine crate, `trapperkeeper-engine`, with the library
`trapperkeeper_engine`, which a pipeline can embed. It holds the rule model, the
tree of a rule's expression, their JSON form and the validation of a rule.
Validation checks the rule's own fields and holds its expression to a depth and
a printed length. The crate also parses a rule's text into the tree and prints a
tree as text. It does not evaluate rules. It depends on no server, store or HTTP
code.

**Map**:

| File                  | What                                                                                                                                                                                       | When to read                                                                                                                                                                           |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Cargo.toml`          | the package name; the dependencies and their features                                                                                                                                      | changing a dependency; why floats parse exactly; why no UUID generator; why the package name ends in `-engine`                                                                         |
| `src/lib.rs`          | the modules and what the crate exports                                                                                                                                                     | what a user of the crate can reach; why `text::Reason` is not exported at the crate root                                                                                               |
| `src/expr.rs`         | the expression tree, a path segment, the type names, and the table `parts!` with the JSON key and the operands of each kind                                                                | the JSON key of a kind of node; the JSON pointer of a node; adding a function or a method                                                                                              |
| `src/rule.rs`         | the rule and the action types                                                                                                                                                              | the JSON form of a rule; the value of a field that JSON omits; why a rule holds the tree and no text                                                                                   |
| `src/limits.rs`       | the limits as constants, each with where its number came from                                                                                                                              | how many scope tags, how long a tag; how long a rule's text may be and how deep it may nest; how deep a tree may be; the cap on token tests in one parse; why the limits are constants |
| `src/validate.rs`     | `validate`, the refusal reasons and the JSON pointer of a refusal                                                                                                                          | which fields of a rule and what of its expression are checked, and in which order; why a rule was refused; adding a check of the expression                                            |
| `src/text/`           | the parser and the printer of rule text                                                                                                                                                    | parsing or printing rule text; a syntax error and its sentence; see `src/text/AGENTS.md`                                                                                               |
| `tests/common/mod.rs` | the strategies for any tree, for a tree that validates and for rules, and the strategy that puts a defect under generated nodes                                                            | generating trees or rules in another test; adding a kind of node to the generated trees                                                                                                |
| `tests/tree.rs`       | the round trip and the refusals of the tree's JSON, as properties over generated trees                                                                                                     | how the JSON form of the tree is tested                                                                                                                                                |
| `tests/text.rs`       | valid and faulty rule texts, the sentence of every reason but `TooComplex`, the round trip and the limits of the parser                                                                    | what an error says for a mistake; adding a faulty text                                                                                                                                 |
| `tests/model.rs`      | the defects of the fields and of the expression, the round-trip and refusal properties of a rule, the edges of the depth and the printed length, and the read-back of chains that validate | how the JSON form of a rule and validation are tested; adding a refusal reason and its defect; how the limits of an expression are tested                                              |

**Invariants**:

- The code that stores a rule, sends it to a sensor or evaluates it assumes
  that `validate` accepted the rule. `validate` checks the id, the sample
  rate and the scope tags, and it holds the expression to `MAX_DEPTH` and its
  printed text to `MAX_TEXT_LENGTH`. It returns the first refusal.
- A new kind of node gets a row in the table `parts!` of `src/expr.rs`, with
  its JSON key and its operands. The compiler asks for the row but does not
  check the key. The property
  `a_node_past_the_depth_cap_is_refused_at_its_place` in `tests/model.rs`
  checks the key of every kind that has an operand.
- The crate depends on serde, serde_json and uuid only.
- Every struct of the model and every node of the tree refuses an unknown
  JSON property.
- A new kind of node is added to the strategy `tree` in
  `tests/common/mod.rs`, so the properties of the tree cover it.
- A strategy that feeds `validate` takes its trees from `valid_expr()`. The
  JSON tests take theirs from `expr()`, which also builds trees that
  `validate` refuses.
- The engine never generates a `rule_id`.
- A new refusal reason of `validate` about a field gets a variant of `Defect`
  in `tests/model.rs`. The property `a_defect_is_refused_at_its_place` checks
  that a valid rule with this defect is refused for it, at its place. A new
  refusal reason about a node of the expression gets a property that places
  its defect under generated nodes with the strategy `around` of
  `tests/common/mod.rs`, and finds the expected place in the JSON of the rule
  with `pointer_to` of `tests/model.rs`.

**Why**:

- The rule format, the form of the tree, how validation answers and the
  limits of an expression: `doc/adr/0033-rule-format.md`.
- The rule language, its kinds of expression and the closed list of functions
  and methods: `doc/adr/0041-the-rule-language.md`.
- The scope tags, their caps and the seen-scope index:
  `doc/adr/0034-scope-tags-and-the-seen-scope-index.md`.
- The id of a rule is a version 7 UUID that the server generates, and the
  title has the `_title` form:
  `doc/adr/0020-internal-ids-and-unique-names.md`.
- The caps on the length and the nesting of a rule's text:
  `doc/adr/0042-the-parser-of-the-rule-language.md`.
- A missing value is a null, and `==` and `!=` with `None` test for it:
  `doc/adr/0005-one-vocabulary-for-failure-behavior.md`.
- Property, behavioral and data-driven tests are preferred:
  `doc/adr/0018-unit-integration-and-end-to-end-tests.md`.
- The crate's directory:
  `doc/adr/0010-one-top-level-directory-per-component.md`.

**How to**: build, lint and test with the commands in the root `AGENTS.md`,
How to. `cargo test -p trapperkeeper-engine` runs this crate alone.
