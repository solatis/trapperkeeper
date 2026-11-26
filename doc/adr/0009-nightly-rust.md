# 0009. Nightly Rust

Date: 2026-11-21.

## Context

A Rust project chooses its release channel, and whether it supports a minimum
compiler version. Miri, the interpreter that checks unsafe code, runs only on
nightly, and so does `std::simd`.

## Decision

The project builds with the nightly channel everywhere, on a developer's
machine and in CI (owner, 2026-11-21).

The nightly floats. `rust-toolchain.toml` names the `nightly` channel with no
date, and there is no minimum supported version (owner, 2026-11-21).

Code turns on an unstable feature only where a decision record names the
feature and gives its reason (owner, 2026-11-26).

## Rejected alternatives

A commit that turns on an unstable feature with a comment, because nothing
would then bound what a floating nightly can break. No unstable feature at all,
because the project chose nightly to use Miri and `std::simd`.

The stable channel, with nightly for the Miri job alone, because nightly is
needed anyway and the owner wants one toolchain.

A nightly pinned to a date. The owner chose the floating nightly and gave no
reason against the pinned one.

A minimum supported Rust version, because it would hold back features for
builders nobody has.

## Consequences

Anyone who builds TrapperKeeper, or a pipeline that embeds the engine crate,
needs a nightly toolchain. The owner expects few Rust users of the engine
crate, and the features that need nightly may be stable before that changes
(owner, 2026-11-21).

A new nightly can break the build without a commit.
