# 0011. Clippy pedantic denied everywhere

Date: 2026-11-26.

## Context

The lint level is a repository-wide convention that every crate inherits from
the workspace, and raising it later means fixing every existing warning at
once.

## Decision

The workspace lints table in the root `Cargo.toml` denies clippy's `all` and
`pedantic` groups and rustc warnings, and every crate inherits it. A single
lint is relaxed per case with a higher-priority entry, never by lowering a
group. CI passes no extra lint flag, so a commit fails the same way on a
developer's machine and in CI.

The table also denies `clippy::undocumented_unsafe_blocks`, a lint outside both
groups, so every `unsafe` block says why it is sound (owner, 2026-11-26).

## Rejected alternatives

The default clippy group alone, as too lenient. Warning locally and denying only
in CI, because a commit could pass locally and fail remotely. Running only the
formatting check and leaving clippy advisory, because warnings then accumulate.

## Consequences

Nothing builds with a known warning while iterating. A new nightly that adds
a warning breaks the build until the warning is fixed.
