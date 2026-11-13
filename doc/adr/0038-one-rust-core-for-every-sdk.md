# 0038. One Rust core for every SDK

Date: 2026-11-13.

## Context

Evaluation happens in the pipeline's process, so every SDK carries an
evaluator, and they must agree (`principles.md`, 4).

## Decision

The design serves Rust, Python, Java and Go (owner, 2026-11-12). One Rust
core serves every SDK (owner, 2026-11-13). Each SDK's native part is compiled
together with its adapters (`adr/0036`).

A native core that can crash the process is an acceptable trade-off. A
validation pass over imported buffers may be needed (owner, 2026-11-12).

No error of ours is raised into a pipeline (`adr/0005`).

How Go embeds the core is decided when a Go SDK is scheduled
(`decisions.md`).

## Rejected alternatives

An evaluator written again in each SDK's language, because agreement would
then rest on fixtures alone.

Isolating the core from the pipeline's process.

## Consequences

A crash of the core is an exception to "harmless when it breaks"
(`principles.md`, 1). An SDK ships native code for each platform.
