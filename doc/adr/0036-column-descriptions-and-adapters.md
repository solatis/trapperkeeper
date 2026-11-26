# 0036. Column descriptions and adapters

Date: 2026-11-26.

## Context

Looking a field up is the most expensive part of evaluating a rule on
objects. Frameworks hold columns in different layouts.

## Decision

The executor reads a view. An adapter builds the view. It looks the fields up in
the cheapest way its framework allows and presents the values as continuous
arrays (owner, 2026-11-13). The executor walks each column as an array of
values, which may lie a fixed stride apart. A view is zero-copy as far as
possible (owner, 2026-11-13).

A view describes each column in our own column description. The description
gives the value type and width, where the values are and how far apart they are,
and how "no value" is marked: never, by a bitmap or by a byte mask. Arrow is
one case of it (owner, 2026-11-13).

The source picks which of these markings a column has. A NaN in a float column
is no value (`adr/0005`) in every source and under every marking (owner,
2026-11-26).

No framework is a model for the description. Arrow is one source among numpy,
pandas, Polars, DuckDB and Rust structs. The layouts of all these sources
count equally, and the description picks abstractions that cover all of them
at once (owner, 2026-11-26).

Data never moves between machines of different byte order, so every value is
in the byte order of the machine (owner, 2026-11-26). A stride is a positive
number of bytes, and every value is aligned for its type (owner, 2026-11-21).

The number types are the signed and unsigned ints of 8 to 64 bits and the
floats of 16, 32 and 64 bits. A boolean is a byte or a bit (owner, 2026-11-21,
2026-11-26). A float16 value is nightly's `f16`, and the engine crate turns on
the unstable feature `f16` for it (owner, 2026-11-26).

A bitmap marks a present value with a set bit and may start at any bit. A byte
mask marks a missing value with a byte that is not zero (owner, 2026-11-21).

A column has one type. Each value of a column is either present or missing,
with no third state (owner, 2026-11-12).

An adapter borrows memory where the framework has it, and fills arrays where
it has none. For dicts and maps an adapter also keeps the rows whose value
had the wrong type, beside the column (owner, 2026-11-13).

An adapter is compiled against the core's traits, in our SDK crates and in a
Rust user's crate, so it can read memory in its own way (owner, 2026-11-13).

The arrow-rs crate serves the Arrow case. arrow-rs imports Arrow data,
validates it and gives typed access to it. A Rust user passes arrow-rs arrays
(owner, 2026-11-12, 2026-11-13).

The column types and the loops are our own. A loop reads a source's memory in
place, with no intermediate column format between the two (owner,
2026-11-13).

## Rejected alternatives

Arrow layouts only, because a numpy boolean array, a masked array's mask and a
column with a stride each need a pass over the data.

arrow-rs arrays as the executor's column type, because owning the
implementation gives more flexibility, and arrow-rs has no stride, no byte
mask and no NaN as null.

Writing the Arrow import ourselves, because dropping arrow-rs would be
anti-pragmatic unless it performs badly.

The machine-learning formats bfloat16 and the 8-bit floats, until a source
needs them (owner, 2026-11-26).

The `half` crate for float16, because it adds a dependency to a crate that
pipelines embed.

A flag on each marking for what a set bit or a non-zero byte means, because no
known source marks a present value with a zero bit or a missing value with a
zero byte (owner, 2026-11-21).

A negative or an unaligned stride, because every strided loop would then read
with an unaligned load and a signed offset. It is added when a source needs it
(owner, 2026-11-21).

A third state per value in a column, because it is incompatible with fast
column-oriented evaluation.

A view that answers one value at a time, because dispatch per row measured
about 20 times the cost of dispatch per chunk.

Building nested Arrow from host objects, because it measured 3 to 8 times the
cost of reading the objects directly.

Converting a convention through a Python library, because pyarrow does the
same pass 10 to 50 times slower than native code.

## Consequences

Every float column costs one pass over its values, which finds the NaNs
(`adr/0005`).

An adapter copies two kinds of numpy view into a new array: a reversed view,
whose stride is negative, and a view into a packed structured array, whose
values are not aligned.

Contiguous memory decides whether a loop vectorizes. A column with a stride
measured about 3 times the contiguous cost.

In Java and Go, host code reads objects and fills arrays, because Rust cannot
walk them cheaply.

The core owns a closed list of cases, and no adapter extends it (owner,
2026-11-13).
