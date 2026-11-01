# 0014. Async everywhere

Date: 2026-11-01.

## Context

The server is bound by I/O: sockets, the store, later the event log. The owner
stated the rule on 2026-11-01, while the first HTTP code was being planned, so
the framework and every I/O dependency are chosen with it.

## Decision

Wherever an async form exists, the code is async, on the tokio runtime:
handlers, I/O, the store calls. Blocking work moves to a blocking pool and
never runs on a runtime thread.

## Rejected alternatives

A threaded blocking server, because every blocking call pins a thread, and the
sensor API is high volume by design.

## Consequences

Every I/O dependency is chosen for its async API. `main` is the runtime's entry
point. Tests that drive the binary run it as a process.
