# engine/src/column/

**What**: the module is the engine's own description of a column: the type of
its values, where they lie in the source's memory, how far apart they lie, and
how the source marks "no value". It evaluates nothing.

**Map**:

| File         | What                                                                                         | When to read                                                                                                                  |
| ------------ | -------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| `values.rs`  | where the values lie: a run with a stride, a run of bits, and the number and boolean types   | what a stride is; building a run from raw memory and what the adapter promises; which number and boolean types a column holds |
| `marking.rs` | the markings, the chunk size, the bitmap of a chunk, the loops that make it and the NaN pass | how a source says "no value"; the bit order of a chunk's bitmap; why a NaN row has no value; adding a marking                 |
| `mod.rs`     | `Column`, its kinds and the check of the row counts                                          | building a column; adding a kind of column                                                                                    |

**Invariants**:

- Every function that builds a description from raw memory is `unsafe` and
  lists what the adapter promises under `# Safety`. A safe function bounds
  every read itself.
- Every `unsafe` block says why it is sound, and the workspace lint
  `undocumented_unsafe_blocks` fails the build otherwise.
- A loop writes every word of the chunk's bitmap and leaves the bits from
  `rows` on at 0, because a later step counts and combines whole words.
- The core owns the closed list of cases, so the public traits of this module
  are sealed.
- A new layout or loop gets a property in `engine/tests/column.rs` that builds
  it from generated data and reads the data back. A property draws the length
  of a generated column from `MAX_ROWS`, which is small under Miri, and the
  property runs under Miri in CI.

**Why**:

- The description, and why no framework is a model for it:
  `doc/adr/0036-column-descriptions-and-adapters.md`.
- A stride is positive and every value is aligned:
  `doc/adr/0036-column-descriptions-and-adapters.md`.
- The chunk of 2048 rows, and one loop per marking that makes the bitmap of a
  chunk: `doc/adr/0035-vectorized-evaluation-with-a-planner.md`.
- A set bit means present, and unknown uses that mask:
  `doc/adr/0005-one-vocabulary-for-failure-behavior.md`.
- The bitmap of a chunk is 64-bit words with the first row in the lowest bit.
  Testing a row is then a shift and a mask, a loop visits the set rows in
  order with `trailing_zeros`, and a logic step works on 64 rows at a time. A
  source bitmap in the same bit order, such as Arrow's, copies with shifts
  alone.
- The loops over values that lie one after the other use `std::simd`, and the
  instruction sets that the build turns on are decided from measurements:
  `doc/adr/0035-vectorized-evaluation-with-a-planner.md`.
- A NaN is a null in every source, so every float column gets a NaN pass after
  its marking: `doc/adr/0005-one-vocabulary-for-failure-behavior.md`.

**How to**:

- `cargo test -p trapperkeeper-engine --test column` runs the properties.
- The CI job `miri` runs them under Miri, with the command in
  `.github/workflows/ci.yml`. Miri runs in CI only, as the root `AGENTS.md`
  says.
