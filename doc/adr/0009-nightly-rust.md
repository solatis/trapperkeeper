# 0009. Nightly Rust

Date: 2026-11-26.

## Context

A Rust project chooses its release channel, and whether it supports a minimum
compiler version. Miri, the interpreter that checks unsafe code, runs only on
nightly, and so does `std::simd`.

## Decision

The project builds with the nightly channel everywhere, on a developer's
machine and in CI (owner, 2026-11-21).

The project pins no nightly to a date. So `rust-toolchain.toml` names the
`nightly` channel with no date, and there is no minimum supported version
(owner, 2026-11-21).

Code turns on an unstable feature only where a decision record names the
feature and gives its reason (owner, 2026-11-26). This record names no unstable
feature, so the first use of `std::simd` needs a record that names it.

## Rejected alternatives

The project rejected letting any commit turn on an unstable feature with only a
comment to justify it, because nothing would then bound what a floating nightly
can break. It also rejected a ban on every unstable feature, because the
project chose nightly to use Miri and `std::simd`.

The project rejected the stable channel with nightly for the Miri job alone,
because nightly is needed anyway and the owner wants one toolchain.

The project rejected a nightly pinned to a date. The owner chose the floating
nightly and gave no reason against the pinned one.

The project rejected a minimum supported Rust version, because it would hold
back features for people who build with an older compiler, and the project has
no such people.

## Consequences

Anyone who builds TrapperKeeper needs a nightly toolchain. A pipeline that
embeds the engine crate needs one as soon as the crate turns on an unstable
feature. The owner expects few Rust users of the engine crate, and the features
that need nightly may be stable before that changes (owner, 2026-11-21).

No commit is needed to follow the compiler, and a new nightly can break the
build without a commit (owner, 2026-11-21).
