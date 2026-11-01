# 0011. Clippy pedantic denied everywhere

Date: 2026-10-31. Status: accepted.

## Context

The lint level is a repository-wide convention that every crate inherits from
the workspace, and raising it later means fixing every existing warning at
once.

## Decision

The workspace lints table in the root `Cargo.toml` denies clippy's `all` and
`pedantic` groups and rustc warnings, and every crate inherits it. A single
lint is relaxed per case with a higher-priority entry, never by lowering a
group. CI passes no extra flag: a commit fails the same way on a developer's
machine and in CI.

## Rejected alternatives

The default clippy group alone, as too lenient. Warning locally and denying
only in CI, because a commit could pass locally and fail remotely. Formatting
checks only with clippy advisory, because warnings then accumulate and lint
stops meaning anything.

## Consequences

Nothing builds with a known warning while iterating. A new stable release that
adds a warning breaks the build until the warning is fixed.
