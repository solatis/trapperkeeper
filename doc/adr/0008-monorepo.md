# 0008. Monorepo

Date: 2026-10-31.

## Context

Cross-cutting changes across the server, the SDKs, the UI and the fixtures
are expected to be common. All components are released together.

## Decision

All components are developed in this one repository, if at all possible. The
exception is an SDK language whose publishing requires a standalone
repository, decided case by case.

## Rejected alternatives

One repository per component, because a cross-cutting change would then span
several pull requests.

## Consequences

A cross-cutting change is one pull request. An SDK that needs its own repository gets a case-by-case record.
