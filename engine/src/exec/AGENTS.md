# engine/src/exec/

**What**: the executor. It binds a `Program` to the column types of one schema
and runs the bound program over each batch of that schema, one chunk of 2048
rows at a time. A run returns, for each rule, the records it matched and the
records it faulted on, each with its fault. It knows no rule text.

**Map**:

| File         | What                                                                                   | When to read                                                                                                             |
| ------------ | -------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| `mod.rs`     | the module and what it exports                                                         | what a caller of the executor can reach                                                                                  |
| `bind.rs`    | `Bound`, the type of a field as bind sees it, and the binding of each step to its loop | how a comparison picks its loop; how a literal converts to the column's type; which comparisons are a type fault at bind |
| `run.rs`     | `Bound::run`, the presence of each field per chunk, and the fault of a faulted record  | how a batch runs chunk by chunk; what `run` checks about a batch                                                         |
| `number.rs`  | the orders as types and the tests of `pack` that compare numbers                       | writing a loop that compares numbers; why an int literal outside the column's type decides every row                     |
| `truth.rs`   | `Truth`, the result of a step for one chunk                                            | how a step's result is held                                                                                              |
| `outcome.rs` | `Outcomes`, `RuleOutcome`, `Outcome` and `Fault`                                       | what a run returns; the faults                                                                                           |

**Invariants**:

- A true row is known, and a faulted row is neither known nor true. A row
  where a field of a comparison has no value is unknown.
- Both operands of `and` and `or` run on every row. The logic step then clears
  the faults of the right operand on the rows that the left operand decided,
  which keeps the written order of the faults without a branch per row.
- A loop writes every word of its bitmap and sets no bit at or past the
  chunk's rows.
- `run` reads no column whose type or row count differs from what bind saw,
  because a loop reads a column's memory as the type that bind picked.
- A literal converts to the type of the column it is compared with, once, at
  bind.

**Why**:

- The program, the result of a step, bind once per schema and the
  caller's interface: `doc/adr/0035-vectorized-evaluation-with-a-planner.md`.
- How numbers convert and compare: `doc/adr/0041-the-rule-language.md`.
- Nulls, NaNs and faults: `doc/adr/0005-one-vocabulary-for-failure-behavior.md`.
- The three outcomes of a record: `doc/adr/0006-conformance-fixtures-are-the-evaluation-spec.md`.

**How to**: `cargo test -p trapperkeeper-engine --test evaluation` runs the
tests of the executor.
