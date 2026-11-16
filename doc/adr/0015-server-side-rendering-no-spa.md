# 0015. Server-side rendering, no single-page application

Date: 2026-11-01.

## Context

The web UI (M8) needs a rendering model, and the web framework chosen now must
not foreclose it. The owner stated the rule on 2026-11-01.

## Decision

The server renders the web UI. Its handlers return HTML through a template
engine that is chosen with M8. There is no single-page application. JavaScript
is allowed ad hoc, for small widgets only.

## Rejected alternatives

A single-page application over the management API, because it doubles the
surface to build and secure, and the UI is low volume.

## Consequences

The framework must let a handler return any body type, and axum does. The
template engine is an open decision in `doc/decisions.md`.
