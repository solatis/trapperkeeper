# 0010. One top-level directory per component

Date: 2026-10-31. Status: accepted.

## Context

The first crate fixed the repository layout, because later crates (the rule
engine, the Rust sensor library) and the non-Rust components follow it.

## Decision

Each component has one top-level directory: `server/` now, later `engine/`,
`sdk/<lang>/`, `web/` and `conformance/`. The Cargo workspace lists its members
one by one. A component directory has its own `AGENTS.md`, and inside
`server/` each module directory gets one as it appears.

## Rejected alternatives

Grouping the Rust crates under `crates/`, because it adds a level and puts the
Rust sensor SDK apart from the other SDKs. A single crate at the root with no
workspace, because the milestone asks for a workspace and moving to one later
renames every path in CI and documents.

## Consequences

Only directories that have code exist; the root `AGENTS.md`, Layout, names the
rest in advance. The workspace member list is explicit rather than a glob.
