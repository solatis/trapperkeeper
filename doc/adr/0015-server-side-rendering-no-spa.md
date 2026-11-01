# 0015. Server-side rendering, no single-page application

Date: 2026-11-01.

## Context

The web UI (M7) needs a rendering model, and the web framework chosen now must
not foreclose it. The owner stated the rule on 2026-11-01.

## Decision

The web UI is rendered on the server: handlers return HTML through a template
engine chosen with M7. There is no single-page application. JavaScript is
allowed ad hoc, for small widgets only.

## Rejected alternatives

A single-page application over the management API, because it doubles the
surface to build and secure, and the UI is low volume.

## Consequences

The framework must let a handler return any body type; axum does. The template
engine (askama or minijinja) is decided with the first page, in M7, and
recorded then.
