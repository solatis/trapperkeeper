# engine/src/plan/

**What**: the planner. It turns the expression trees of a rule set into one
`Program`: a flat list of steps, children before parents, that the executor
runs from first to last. Each step stores its result under its index in the
list, its _slot_, and later steps read earlier results by slot. The planner
also lists the fields that the rules read, as a tree of paths, and gives each
field a number. It reads rules only and knows nothing about columns or types;
`src/exec/` binds a program to the columns of a schema and runs it.

**Map**:

| File         | What                                                                  | When to read                                                                                                                                                                    |
| ------------ | --------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `mod.rs`     | the module and what it exports                                        | what a caller of the planner can reach                                                                                                                                          |
| `fields.rs`  | `FieldTree` and `FieldNode`: the paths that the rules read, as a tree | how fields get their numbers; how an adapter, the code that finds the columns of a source, learns which columns to find and in which order                                      |
| `program.rs` | `Program`, its steps, `plan` and `Unsupported`                        | what a step is; how a comparison becomes a step; which expressions this version supports; how `1 < 2` is computed early; how rules share a step; how `x in [...]` becomes steps |

**Invariants**:

- A step reads only slots before its own.
- Two equal steps are one slot, so two rules that both test `x > 5` share one
  result. Two float literals count as equal when their bits are equal.
- Fields are numbered in the order of a depth-first walk over the tree, with
  the children of a node in the order in which the rules first read them. A
  caller passes its columns in that order.
- A comparison step has a field on its left. The planner turns `5 < x` into
  `x > 5` with `Comparison::flip`.
- `plan` refuses a rule set in which any rule uses an expression this version
  cannot run, and names the first such rule. No rule runs with a part
  skipped.

**Why**:

- One program for the whole rule set, so rules share steps and columns, and
  the planner as the place that lists the fields. The fields form a tree, so
  an adapter looks up a prefix such as `user` once for all the fields below
  it: `doc/adr/0035-vectorized-evaluation-with-a-planner.md`.
- `None` used as a condition is unknown, and a number used as a condition is
  a fault: `doc/adr/0005-one-vocabulary-for-failure-behavior.md` and
  `doc/adr/0041-the-rule-language.md`.

**How to**: a new kind of expression needs a `Step` variant, an arm in
`Planner::expr`, an arm in `bind_step` of `src/exec/bind.rs`, an arm in the
reference interpreter of `tests/evaluation.rs`, and a case in the strategy
`condition` of that file, which generates the rules that its property tests
run.
