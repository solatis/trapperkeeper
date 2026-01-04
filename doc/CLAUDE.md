# doc/

Technical documentation for TrapperKeeper architecture, implementation patterns, and operations.

## Files

| File                      | What                             | When to read                                     |
| ------------------------- | -------------------------------- | ------------------------------------------------ |
| `README.md`               | Documentation structure overview | Understanding navigation, hub-spoke patterns     |
| `error-handling-index.md` | Cross-cutting error handling     | Understanding failure modes, recovery strategies |
| `observability-index.md`  | Cross-cutting observability      | Implementing logging, metrics, tracing           |
| `performance-index.md`    | Cross-cutting performance        | Optimizing latency, throughput, resource usage   |
| `security-index.md`       | Cross-cutting security           | Implementing auth, encryption, threat mitigation |
| `validation-index.md`     | Cross-cutting validation         | Implementing input validation, sanitization      |

## Subdirectories

| Directory          | What                             | When to read                                        |
| ------------------ | -------------------------------- | --------------------------------------------------- |
| `_meta/`           | Documentation standards, tooling | Creating docs, using templates, running validation  |
| `01-principles/`   | Architectural principles         | Understanding design philosophy, testing strategy   |
| `02-architecture/` | System architecture              | Understanding services, API design, SDK model       |
| `03-data/`         | Data schemas, storage            | Understanding events, identifiers, timestamps       |
| `04-rule-engine/`  | Rule expression language         | Understanding field paths, type coercion, operators |
| `05-performance/`  | Performance optimization         | Understanding cost models, sampling, batching       |
| `06-security/`     | Security architecture            | Understanding auth, TLS, encryption                 |
| `07-validation/`   | Validation architecture          | Understanding 4-layer validation model              |
| `08-resilience/`   | Error handling, degradation      | Understanding failure modes, logging, monitoring    |
| `09-operations/`   | Configuration, deployment        | Understanding config, database, CLI, migrations     |
| `10-integration/`  | Module structure                 | Understanding package organization, dependencies    |
| `scripts/`         | Validation tooling               | Running doc validation, debugging failures          |
