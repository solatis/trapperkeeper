# engine/src/column/

**What**: the module is the engine's own description of a column: the type of
its values, where they lie in the source's memory, how far apart they lie, and
how the source marks "no value". It also holds the loops that turn a marking
into the bitmap of a chunk, and the pass that clears the rows whose float is a
NaN. It also describes text columns and list columns, which share their
layouts of rows. The elements of a list are a column of their own. It
evaluates no rule.

**Map**:

| File         | What                                                                                             | When to read                                                                                                                                                                                       |
| ------------ | ------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `values.rs`  | where the values lie: a run with a stride, a run of bits, and the number and boolean types       | what a stride is; building a run from raw memory and what the adapter promises; which number and boolean types a column holds                                                                      |
| `marking.rs` | the markings, the chunk size, the bitmap of a chunk, the loops that make it and the NaN pass     | how a source says "no value"; the bit order of a chunk's bitmap; why a NaN row has no value; adding a marking                                                                                      |
| `layout.rs`  | the offsets and the fixed-width layouts, generic over what their rows point into                 | which offset types are supported; what an adapter promises about offsets; how a numpy view with a step, such as `a[::2]`, is described; adding a layout of rows                                    |
| `text.rs`    | the trait `Texts` and the layouts of a text column                                               | writing a loop over text; what an adapter promises about text bytes and pieces; the layout of a view; why a numpy cell loses its trailing zeros; why UTF-32 is read as units; adding a text layout |
| `list.rs`    | the trait `Lists`, the layouts of a list column, the shape of a column and the column per record | writing a loop over a list; lists of texts, lists of lists and masked arrays of masked arrays; adding a list layout                                                                                |
| `mod.rs`     | `Column`, its kinds and the check of the row counts                                              | building a column; adding a kind of column                                                                                                                                                         |

**Invariants**:

- A constructor whose caller promises something that the constructor does
  not check, such as raw memory that is readable, offsets in order or bytes
  that are UTF-8, is `unsafe` and states the promise under `# Safety`. A debug
  build checks the parts of the promise that it can check, such as order,
  bounds and UTF-8. A safe function bounds every read itself.
- Every `unsafe` block says why it is sound, and the workspace lint
  `undocumented_unsafe_blocks` fails the build otherwise.
- A loop that makes the bitmap of a chunk writes all its words, and sets no
  bit at or past the chunk's row count `rows`, because a later step counts
  and combines whole words.
- The engine owns the closed list of cases, so a public trait of this module,
  such as `Float`, is sealed.
- A layout reads each text and each element where the source holds it, and
  copies none.
- A loop over text is written against `Texts`.
- A list borrows its element column, and no description allocates.
- The element column of a list has its own marking for the elements without
  a value, and the list column's marking says which records have no list.
- The records of a column-per-record list share one shape, and each may have
  its own layout and marking, so a loop dispatches once per record.
- `Text::Utf32` is the one layout that is not UTF-8, and no function gives a
  `&str` or a `char` for it.
- A new layout or loop gets a property in `engine/tests/column.rs` that builds
  it from generated data and reads the data back. The property runs under
  Miri in the nightly workflow `miri.yml`.

**Why**:

- The description, and why no framework is a model for it:
  `doc/adr/0036-column-descriptions-and-adapters.md`.
- The adapter promises that the memory it describes is valid, and a release
  build does not validate it: `doc/adr/0038-one-rust-core-for-every-sdk.md`.
- The text layouts, the layouts that text and lists share, one trait per kind
  and no copies: `doc/adr/0036-column-descriptions-and-adapters.md`.
- The list layouts: `doc/adr/0036-column-descriptions-and-adapters.md`.
- A column per record nests to any depth and copies no value or mask, at the
  cost of one dispatch per record:
  `doc/adr/0036-column-descriptions-and-adapters.md`.
- The cells of numpy's `U` type are UTF-32, an exception to UTF-8 everywhere
  else: `doc/adr/0041-the-rule-language.md`.
- A stride is positive, every value is aligned, and the rows of a fixed-width
  layout do not overlap:
  `doc/adr/0036-column-descriptions-and-adapters.md`.
- A bitmap marks a present value with a set bit and may start at any bit, a
  byte mask marks a missing value with a byte that is not zero, and no flag
  turns either meaning around:
  `doc/adr/0036-column-descriptions-and-adapters.md`.
- The chunk of 2048 rows, and one loop per marking that makes the bitmap of a
  chunk: `doc/adr/0035-vectorized-evaluation-with-a-planner.md`.
- A set bit means present. A comparison on a row whose bit is clear gives
  unknown, the null of a boolean:
  `doc/adr/0005-one-vocabulary-for-failure-behavior.md`.
- The loops over values that lie one after the other use `std::simd`, and the
  instruction sets that the build turns on are decided from measurements:
  `doc/adr/0035-vectorized-evaluation-with-a-planner.md`.
- A NaN is a null in every source, so every float column gets a NaN pass after
  its marking: `doc/adr/0005-one-vocabulary-for-failure-behavior.md`.

**How to**:

- `cargo test -p trapperkeeper-engine --test column` runs the properties.
- The workflow `.github/workflows/miri.yml` runs them under Miri, nightly
  and on demand. The root `AGENTS.md` says when an agent may start it.
