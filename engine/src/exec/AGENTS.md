# engine/src/exec/

**What**: the executor. A `Program` from `src/plan/` is untyped: `x > 5` does
not say whether `x` is an `i8` or an `f64`. `Program::bind` looks at the
column types of one schema and picks, for each comparison, a loop compiled for
exactly those types. `Bound::run` then runs the bound program over each batch
of that schema, 2048 rows at a time, and returns for each rule the records it
matched and the records it could not evaluate, each with its fault. The
executor knows no rule text.

**Map**:

| File         | What                                                                                                                                    | When to read                                                                                                                                             |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `mod.rs`     | the module and what it exports                                                                                                          | what a caller of the executor can reach                                                                                                                  |
| `bind.rs`    | `Program::bind`, `Bound`, the column type of a field, and the choice of loop for each step                                              | how a comparison gets its loop; how a literal converts to the column's type; which comparisons fault and which can never be equal; how booleans are read |
| `run.rs`     | `Bound::run`: the check of a batch, the loop over chunks, and the lookup of a faulted record's fault                                    | how a batch runs chunk by chunk; what `run` checks about a batch                                                                                         |
| `number.rs`  | the six operators as types, such as `Less`, the loops that compare numbers, and the type in which two number columns compare (`Common`) | writing a loop that compares numbers; why `x < 300` on a `u8` column needs no loop over values; in which type `x < y` compares                           |
| `search.rs`  | substring search and list search, and the choice between them from the type of the right side of `in`                                   | how `x in y` runs when `y` is a field; why the same rule searches a text or a list                                                                       |
| `text.rs`    | the loops that compare texts, in the encodings of the columns and across UTF-8 and UTF-32                                               | comparing a text with a literal or another text; how a UTF-8 text compares with a UTF-32 text                                                            |
| `result.rs`  | `StepResult`, what one step computed for each row of a chunk, and the `not`, `and` and `or` that combine results                        | how true, false, unknown and fault are stored; how `and` and `or` ignore a fault on the right                                                            |
| `member.rs`  | the sets of a membership list, built at bind for the column's type, and the tests that look a row up in them                            | how `x in [...]` runs; how a long list is searched                                                                                                       |
| `outcome.rs` | `Outcomes`, `RuleOutcome`, `Outcome` and `Fault`                                                                                        | what a run returns; the kinds of fault                                                                                                                   |

**Invariants**:

- Each row of a step's result is true, false, unknown or a fault. A row where
  a field of a comparison has no value is unknown.
- `and` and `or` compute both operands on every row, with no branch per row.
  They then drop the faults of the right operand on the rows where the left
  operand already decided the result: in `a and b`, a fault of `b` counts
  only where `a` is not false. This keeps the written order of ADR-0005.
- A loop writes every word of its bitmap and sets no bit at or past the
  chunk's last row.
- `run` refuses a batch whose column types or row counts differ from what
  bind saw, because the loops read a column's memory as the type that bind
  picked.
- Bind picks the meaning of `x in y` with a field `y` from the type of `y`. It
  searches a text for a substring and a list for an element.
- A literal converts to the type of the column it is compared with, once, at
  bind.

**Why**:

- The program, the three bitmaps of a step's result, bind once per schema
  and the caller's interface: `doc/adr/0035-vectorized-evaluation-with-a-planner.md`.
- How values convert and compare, which types can be equal, and how a UTF-8
  text compares with a UTF-32 text: `doc/adr/0041-the-rule-language.md`.
- Nulls, NaNs and faults: `doc/adr/0005-one-vocabulary-for-failure-behavior.md`.
- Why substring search uses `regex`: `engine/Cargo.toml`.
- The three outcomes of a record: `doc/adr/0006-conformance-fixtures-are-the-evaluation-spec.md`.

**How to**:

- `cargo test -p trapperkeeper-engine --test evaluation` runs the tests of the
  executor.
- A new number type needs a variant of `Ints` or `Floats` and a line in the
  list of `numbers!` in `src/column/values.rs`. The list writes the type's
  `Number` impl, its `NumberType` variant and its arm in `with_int!`,
  `with_float!` and `with_number!`, so the comparisons of `bind.rs` cover the
  type with no change there.
