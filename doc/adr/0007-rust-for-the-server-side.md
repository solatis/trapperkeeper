# 0007. Rust for the server side

Date: 2026-10-31.

## Context

The web server and the APIs needed an implementation language. Performance is
extremely important for this project, and the option of a shared evaluator
core bound into other languages had to stay open.

## Decision

The web server and the APIs are written in Rust. Correctness counts as much as
performance. A garbage-collected language is less suited to FFI-style use. Rust
lets the server and every SDK share one evaluator core, as `adr/0038` decides.
The owner's background is heavily C++ and Haskell. Rust is gaining ground for
this kind of project with LLM providers and the wider community (Anthropic,
OpenAI, Cloudflare, the Linux kernel).

## Rejected alternatives

Go, as a garbage-collected language less suited to FFI-style use.

## Consequences

The SDK languages are a separate decision, which `adr/0038` records.
`overview.md`, Integration surface, states the order of the SDKs. The server
crate cites this record.
