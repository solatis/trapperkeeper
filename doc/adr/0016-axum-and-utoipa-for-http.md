# 0016. axum and utoipa for HTTP and the API contract

Date: 2026-11-01.

## Context

The server needs a web framework, and the milestone asks that the API contract
be a schema generated from the handlers. The owner prefers libraries that
compose over a framework that owns the application, wants custom middleware to
be possible, and accepts being coupled to the framework's router (2026-11-01).

## Decision

axum on tokio, with middleware as tower layers; tower-http supplies the common
ones. utoipa derives the OpenAPI document from `utoipa::path` annotations on
the handlers, utoipa-axum registers a handler and its description in one
`routes!` call, and the document is served at `/openapi.json` from the first
endpoint on.

## Rejected alternatives

actix-web, because its middleware and error types are its own rather than
tower's. rocket, as the most opinionated: its own conventions for everything
and a smaller ecosystem. poem with poem-openapi, which has the schema built in
but is a less common choice. Deferring the schema to the first real API,
because the milestone counts the contract as a decision this milestone forces.

## Consequences

Every handler is annotated as it arrives, or it is missing from the contract.
The router type is axum's.
