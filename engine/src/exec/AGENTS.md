# engine/src/exec/

**What**: the executor. It binds a `Program`, the planned rule set of
`src/plan/`, to the column types of one schema and runs the bound program over
each batch of that schema, one chunk of 2048 rows at a time. A run returns, for each rule, the records it matched and the
records it faulted on, each with its fault. It knows no rule text.

**Map**:

| File         | What                                                                                                                                                    | When to read                                                                                                                                          |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| `mod.rs`     | the module and what it exports                                                                                                                          | what a caller of the executor can reach                                                                                                               |
| `bind.rs`    | `Bound`, the type of a field as bind sees it, and the binding of each step to its loop                                                                  | how a comparison picks its loop; how a literal converts to the column's type; which comparisons are a type fault at bind; how a boolean field is read |
| `run.rs`     | `Bound::run`, the presence of each field per chunk, and the fault of a faulted record                                                                   | how a batch runs chunk by chunk; what `run` checks about a batch                                                                                      |
| `number.rs`  | the six comparisons as types, such as `Less`, the tests that `pack` of `src/column/` runs to compare numbers, and the common type of two number columns | writing a loop that compares numbers; why an int literal outside the column's type decides every row; in which type two number fields compare         |
| `result.rs`  | `StepResult`, the result of a step for one chunk                                                                                                        | how a step's result is held                                                                                                                           |
| `outcome.rs` | `Outcomes`, `RuleOutcome`, `Outcome` and `Fault`                                                                                                        | what a run returns; the faults                                                                                                                        |

**Invariants**:

- A row of a step's result is true, false, unknown or faulted, and a row is
  known when it is true or false. A row where a field of a comparison has no
  value is unknown.
- `and` and `or` run both operands on every row. Their step then clears the
  faults of the right operand on the rows that the left operand already
  decided. A fault of the right operand thus counts only where the left one
  left the result open, which is the written order of ADR-0005. No loop
  branches per row.
- A loop writes every word of its bitmap and sets no bit at or past the
  chunk's rows.
- `run` reads no column whose type or row count differs from what bind saw,
  because a loop reads a column's memory as the type that bind picked.
- A literal converts to the type of the column it is compared with, once, at
  bind.

**Why**:

- The program, the result of a step, bind once per schema and the caller's
  interface: `doc/adr/0035-vectorized-evaluation-with-a-planner.md`.
- How numbers convert and compare: `doc/adr/0041-the-rule-language.md`.
- Nulls, NaNs and faults: `doc/adr/0005-one-vocabulary-for-failure-behavior.md`.
- The three outcomes of a record: `doc/adr/0006-conformance-fixtures-are-the-evaluation-spec.md`.

**How to**:

- `cargo test -p trapperkeeper-engine --test evaluation` runs the tests of the
  executor.
- A new number type needs a variant of `Ints` or `Floats` and a line in the
  list of `numbers!` in `src/column/values.rs`. The list writes the type's
  `Number` impl, its `NumberType` variant and its arm in `with_int!`,
  `with_float!` and `with_number!`, so the comparisons of `bind.rs` cover the
  type with no change there.
