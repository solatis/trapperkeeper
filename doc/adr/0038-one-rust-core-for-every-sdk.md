# 0038. One Rust core for every SDK

Date: 2026-11-26.

## Context

Evaluation happens in the pipeline's process, so every SDK carries an
evaluator. The evaluators of all SDKs must give the same outcomes
(`principles.md`, 4).

## Decision

The design serves Rust, Python, Java and Go (owner, 2026-11-12). One Rust
core serves every SDK (owner, 2026-11-13). Each SDK's native part is compiled
together with its adapters (`adr/0036`).

A native core that can crash the process is an acceptable trade-off (owner,
2026-11-12).

An adapter promises that the memory it describes is valid, such as offsets
that are in range and text bytes that are UTF-8, and a release build of the
core does not check it. The project owns the adapters of its SDKs, and this
explicit contract keeps the loops fast (owner, 2026-11-21). The constructors
that take such memory are `unsafe`, so a Rust user who writes an adapter makes
the same promise.

By default no error of ours is raised into a pipeline (`adr/0005`).

How Go embeds the core is decided when a Go SDK is scheduled
(`decisions.md`).

## Rejected alternatives

An evaluator written again in each SDK's language, because agreement would
then rest on fixtures alone.

Isolating the core from the pipeline's process, because isolation costs
copies, as the loops read the memory of a source in place (`adr/0036`), and it
costs a crossing per batch. It guards against a crash that correct SDKs do not
cause (owner, 2026-11-16).

A pass in the core that validates every text column, because every batch
would pay it, also where the source has already validated.

A safe constructor that validates, beside an unsafe one that does not. The
owner gave no reason beyond the decision that the adapter promises the memory
is valid.

An executor of our own written in pure Java, because one Rust core serves every
SDK (owner, 2026-11-15). A native integration with a host engine, such as an
executor for Spark built for performance, stays possible, and `decisions.md`
keeps that option open.

## Consequences

A crash of the core is an exception to "harmless when it breaks"
(`principles.md`, 1). An SDK ships native code for each platform.

An adapter that breaks its promise causes undefined behavior in a release
build. A debug build checks the parts of the promise that it can check, such as
the order of offsets and UTF-8.
