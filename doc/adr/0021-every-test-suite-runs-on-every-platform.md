# 0021. Every test suite runs on every platform

Date: 2026-11-02.

## Context

The store gained Postgres and MariaDB, and the CI runners do not offer their
servers the same way: GitHub runs service containers on Linux runners only,
the macOS runner has neither database nor Docker, and the Windows ARM64 runner
has no Docker. The owner stated the rule while the backends were planned
(owner, 2026-11-02).

## Decision

Every test runs on every platform CI covers, so the suites stay as identical
as possible (owner, 2026-11-02). A platform skips a test only where a
dependency has no build for it, and the CI file says which and why.

CI tests the latest stable release of each external dependency, and its latest
LTS release where the project publishes LTS releases (owner, 2026-11-02).
Postgres publishes none and supports each major for five years, so CI tests
its latest major. MariaDB publishes LTS releases, so CI tests its latest LTS.

## Rejected alternatives

The database tests on Linux amd64 only, because platform differences would go
unseen and the suites would differ. Pinned majors, because they age without
anyone noticing. MariaDB's newest rolling release, because operators deploy
the LTS.

## Consequences

CI provisions each database per platform: containers on Linux, the package
manager elsewhere. The macOS and Windows runs are slower. Windows aarch64 runs
SQLite only, because neither Postgres nor MariaDB ships a build for it and the
runner has no Docker. A new major can turn CI red without a commit. A test
helper fails a CI run in which an expected database is absent, because a
skipped test passes.
