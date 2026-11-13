# 0036. Column descriptions and adapters

Date: 2026-11-13.

## Context

Looking a field up is the most expensive part of evaluating a rule on
objects, and frameworks hold columns in different layouts.

## Decision

The executor reads a view. An adapter builds the view. It looks the fields up
in the cheapest way its framework allows and presents the values as
continuous arrays (owner, 2026-11-13). A view is zero-copy as far as possible
(owner, 2026-11-13).

A view describes each column in our own column description. The description
gives the value type and width, where the values are and how far apart, and
how "no value" is marked: never, by a bitmap, by a byte mask or by NaN. Arrow
is one case of it (owner, 2026-11-13).

A source says what "no value" is. A NaN is missing in a pandas float column
and a value in a Polars or Arrow column (owner, 2026-11-13).

A column has one type, and a value is there or it is not (owner,
2026-11-12).

An adapter borrows memory where the framework has it, and fills arrays where
it has none. For dicts and maps an adapter also keeps the rows whose value
had the wrong type, beside the column (owner, 2026-11-13).

An adapter is compiled against the core's traits, in our SDK crates and in a
Rust user's crate, so it can read memory in its own way (owner, 2026-11-13).

arrow-rs serves the Arrow case. It imports Arrow data, validates it, gives
typed access, and is the type a Rust user passes (owner, 2026-11-12,
2026-11-13).

The column types and the loops are our own. A loop reads a source's memory in
place, with no intermediate column format between the two (owner,
2026-11-13).

## Rejected alternatives

Arrow layouts only, because a numpy boolean, a masked array's mask, a column
with a stride and NaN as null each need a pass over the data.

arrow-rs arrays as the executor's column type, because owning the
implementation gives more flexibility, and arrow-rs has no stride, no byte
mask and no NaN as null.

Writing the Arrow import ourselves, because dropping arrow-rs would be
anti-pragmatic unless it performs badly.

A third state per value in a column, because it is incompatible with fast
column-oriented evaluation.

A view that answers one value at a time, because dispatch per row measured
about 20 times the cost of dispatch per chunk.

Building nested Arrow from host objects, because it measured 3 to 8 times the
cost of reading the objects directly.

Converting a convention through a Python library, because pyarrow does the
same pass 10 to 50 times slower than native code.

## Consequences

The same numbers can give a different outcome through pandas than through
Polars.

Contiguous memory decides whether a loop vectorizes. A column with a stride
measured about 3 times the contiguous cost.

In Java and Go, host code reads objects and fills arrays, because Rust cannot
walk them cheaply.

The cases of the description are fixed when the description is built.
