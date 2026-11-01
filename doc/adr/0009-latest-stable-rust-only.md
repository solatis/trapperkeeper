# 0009. Latest stable Rust only

Date: 2026-10-31.

## Context

A Rust project chooses whether to support a minimum compiler version below the
current stable release.

## Decision

The project targets the latest stable Rust release only. There is no minimum
supported version below it: code may use any feature as soon as it is stable,
and CI builds with the current stable toolchain. `rust-toolchain.toml` names
the `stable` channel with no version.

## Rejected alternatives

A minimum supported Rust version, because it would hold back features for
builders nobody has.

## Consequences

Anyone building TrapperKeeper needs an up-to-date toolchain.
