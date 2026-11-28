# engine/src/plan/

**What**: the planner. It turns a rule set into one `Program`, a flat list of
steps over numbered slots, and lists the fields that the rules read as a tree
of paths. It reads rules and knows no column. `exec` binds a program to the
columns of a schema and runs it.

**Map**:

| File         | What                                                                     | When to read                                                                                                                                               |
| ------------ | ------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `mod.rs`     | the module and what it exports                                           | what a caller of the planner can reach                                                                                                                     |
| `fields.rs`  | `FieldTree` and `FieldNode`: the tree of the paths and the field numbers | how an adapter, the code that describes the columns of a source, learns which fields to read and in which order; why a path under a quantifier is no field |
| `program.rs` | `Program`, its steps, `plan` and `Unsupported`                           | what a step is; how a comparison becomes a step; which kinds this version plans; how two literals fold; how rules share a step                             |

**Invariants**:

- A step reads only slots before its own, and the slot of a step is its place
  in the list.
- Two equal steps are one slot, so rules that share a comparison or a
  subexpression share its result. Two float literals are equal when their bits
  are.
- The field numbers are the order of a depth-first walk over the tree, with
  the children of a node in the order in which the rules first read them. A
  caller hands its columns over in that order.
- A comparison step holds its field on the left. The planner swaps a literal
  on the left and flips the operator.
- `plan` refuses a rule set with a kind of node that it does not plan, and
  names the first rule that holds one, so no rule runs with a part skipped.

**Why**:

- One program per rule set, and the planner as the place that lists the
  fields. The fields form a tree of paths, so an adapter looks up a prefix
  that several paths share once:
  `doc/adr/0035-vectorized-evaluation-with-a-planner.md`.
- A null where a condition is expected is unknown, and a literal that is no
  boolean there is a fault: `doc/adr/0005-one-vocabulary-for-failure-behavior.md`
  and `doc/adr/0041-the-rule-language.md`.

**How to**: a new kind of expression gets a `Step` variant, an arm in
`Planner::expr`, an arm in `bind_step` of `src/exec/bind.rs`, an arm in the
reference interpreter of `tests/evaluation.rs`, and a case in the strategy
`condition` of that file, which generates the rules of its properties.
