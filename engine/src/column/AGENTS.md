# engine/src/column/

**What**: the module is the engine's own description of a column: the type of
its values, where they lie in the source's memory, how far apart they lie, and
how the source marks "no value". It evaluates nothing.

**Map**:

| File         | What                                                                                       | When to read                                                                                                                  |
| ------------ | ------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------- |
| `values.rs`  | where the values lie: a run with a stride, a run of bits, and the number and boolean types | what a stride is; building a run from raw memory and what the adapter promises; which number and boolean types a column holds |
| `marking.rs` | the markings of "no value"                                                                 | how a source says "no value"; adding a marking                                                                                |
| `mod.rs`     | `Column`, its kinds and the check of the row counts                                        | building a column; adding a kind of column                                                                                    |

**Invariants**:

- Every function that builds a description from raw memory is `unsafe` and
  lists what the adapter promises under `# Safety`. A safe function bounds
  every read itself.
- Every `unsafe` block says why it is sound, and the workspace lint
  `undocumented_unsafe_blocks` fails the build otherwise.

**Why**:

- The description, and why no framework is a model for it:
  `doc/adr/0036-column-descriptions-and-adapters.md`.
- A stride is positive and every value is aligned:
  `doc/adr/0036-column-descriptions-and-adapters.md`.
