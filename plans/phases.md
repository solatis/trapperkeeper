# TrapperKeeper Implementation Phases

Gap analysis between specifications (`doc/`) and current implementation, with proposed phases starting from Phase 5.

## Completed Phases

| Phase | Name            | Deliverables                                                                  |
| ----- | --------------- | ----------------------------------------------------------------------------- |
| 1     | Foundation      | go.mod, proto definitions, types, code generation                             |
| 2     | Rule Engine     | fieldpath resolution, coercion, compilation, evaluation (1743 lines of tests) |
| 3     | Database        | migrations, connection pooling, query management (SQLite + PostgreSQL)        |
| 4     | gRPC Sensor API | HMAC auth, SyncRules, ReportEvents, JSONL persistence, CLI (sensor-api)       |

## Phase 4 Remaining Work

Phase 4 shipped the gRPC service skeleton but left several items incomplete. These become Phase 5.

| Item                            | Current State                                                    | Spec Reference                    |
| ------------------------------- | ---------------------------------------------------------------- | --------------------------------- |
| Rule evaluation in ReportEvents | `matched_rule_count` hardcoded to 0, no calls to `rulesEngine`   | 04-rule-engine, 10-integration    |
| Engine methods                  | `rules.Engine` is empty struct stub, no Evaluate/Compile methods | 04-rule-engine                    |
| `if_none_match` proto field     | SyncRulesRequest missing field, ETAG comparison impossible       | 02-architecture (ETAG-based sync) |
| `rejected_count` in response    | ReportEventsResponse field never populated                       | sensor_api.proto                  |
| `migrate` CLI subcommand        | Referenced but not implemented                                   | 09-operations                     |
| Tests for auth package          | Zero test files in `internal/core/auth/`                         | 01-principles (integration-first) |
| Tests for API handlers          | Zero test files in `internal/core/api/`                          | 01-principles (integration-first) |
| Tests for gRPC server           | Zero test files in `internal/core/server/`                       | 01-principles (integration-first) |
| Tests for database layer        | Zero test files in `internal/core/db/`                           | 01-principles (integration-first) |

## Gap Analysis

| Gap                     | Spec Location                   | Current State             |
| ----------------------- | ------------------------------- | ------------------------- |
| Web UI service          | 02-architecture, 09-operations  | Not started               |
| Go SDK                  | 02-architecture, 10-integration | Empty directory           |
| Python SDK              | 10-integration                  | Not started               |
| Java SDK                | 10-integration                  | Not started               |
| Rule CRUD APIs          | Implied by Web UI               | No handlers               |
| User/session management | Schema in 003, 06-security      | Tables exist, no handlers |
| API key management      | 06-security                     | Tables exist, no admin UI |
| Prometheus metrics      | 08-resilience, 09-operations    | Not implemented           |
| Health endpoints        | 09-operations                   | Partial (grpc health)     |
| Integration tests       | 01-principles                   | Empty directory           |

## Proposed Phases

### Phase 5: Rule Evaluation Wiring + Test Coverage

**Rationale:** Phase 4 shipped the gRPC service skeleton but without rule evaluation wiring, the `migrate` subcommand, ETAG optimization, or tests. These must be completed before building SDKs that depend on correct server behavior.

**Deliverables:**

- `internal/rules/engine.go` -- Stateless Engine wrapping package-level Compile/Evaluate as convenience methods (no caching, caller manages rule lifecycle)
- `internal/rules/fieldpath.go` -- Refactor Resolve() to accept `any` (pre-unmarshaled tree) instead of `json.RawMessage`, removing JSON dependency from evaluation hot path
- `internal/rules/evaluate.go` -- Update Evaluate() signature: payload parameter changes from `json.RawMessage` to `any`
- Conversion layer: pb.Rule -> types.Rule conversion, reusing existing SyncRules DB-to-proto path
- `internal/core/api/report_events.go` -- Wire rule evaluation into processEvent, populate matched_rule_count and event_rule_matches. Use `structpb.Struct.AsMap()` for payload conversion.
- `proto/trapperkeeper/sensor/v1/sensor_api.proto` -- Add `if_none_match` field to SyncRulesRequest
- `internal/core/api/sync_rules.go` -- Implement ETAG comparison using if_none_match
- `internal/core/api/report_events.go` -- Populate rejected_count (counts EVENT_STATUS_REJECTED only, i.e. validation failures)
- `cmd/trapperkeeper/cmd/migrate.go` -- Cobra subcommand wiring db.MigrateUp
- `internal/core/auth/auth_test.go` -- Integration tests for HMAC auth flow
- `internal/core/api/api_test.go` -- Integration tests for SyncRules and ReportEvents
- `internal/core/server/grpc_test.go` -- Server lifecycle tests
- `internal/core/db/db_test.go` -- Migration and query tests

**Key Design Points:**

- Engine is stateless pass-through -- wraps Compile/Evaluate, caller handles rule fetching and batch-level compilation
- Rule engine operates on general-purpose in-memory data structures (`map[string]any`/`[]any`), not serialized JSON bytes. Callers convert from their source format (structpb, JSON, DataFrame row) before evaluation. This is performance-critical.
- Conversion path for evaluation: DB row -> pb.Rule (existing) -> types.Rule (new) -> CompiledRule (existing Compile)
- Rule evaluation happens per-event within ReportEvents, matches stored in event_rule_matches table
- ETAG comparison returns empty rules array when etag matches (not gRPC 304)
- `migrate` subcommand reuses existing db.MigrateUp with embedded FS
- Tests use both SQLite (fast CI) and testcontainers Postgres (thorough validation)
- rejected_count counts EVENT_STATUS_REJECTED only (validation failures), not rule-action drops or DB errors

### Phase 6: Observability

**Rationale:** Metrics and structured logging must exist before deploying the SDK or web UI. The spec's 08-resilience document defines monitoring as a core architectural concern with specific alert thresholds (database errors: alert on ANY, network: >10 consecutive, 5xx: >10%). Retrofitting instrumentation after multiple services are live is expensive and error-prone.

**Deliverables:**

- `internal/core/metrics/prometheus.go` -- Prometheus registry and collectors
- `/api/v1/stats/prometheus` endpoint on sensor-api (extended to web-ui in Phase 8)
- Metrics implementation:
  - `tk_events_total` (counter, labels: status)
  - `tk_rule_matches_total` (counter, labels: rule_id, action)
  - `tk_errors_total` (counter, labels: category per 08-resilience taxonomy)
  - `tk_request_duration_seconds` (histogram, labels: endpoint)
  - `tk_database_errors_total` (counter)
  - `tk_network_failure_streak` (gauge)
- Structured logging audit (verify slog usage, no fmt.Println)
- Alert threshold documentation per 08-resilience specs

**Key Design Points:**

- Six error categories: network, type_coercion, missing_field, database, protocol, validation
- Alert thresholds: database errors ANY, network failures >10 consecutive, 5xx >10%
- DEBUG level for expected errors (coercion, missing fields)
- Instrument sensor-api first; web-ui picks up the same metrics package in Phase 8

### Phase 7: Go SDK

**Rationale:** Required before Python/Java SDKs since they share design patterns. Enables integration testing against real sensor-api. Observability from Phase 6 provides visibility into SDK-server interactions from day one.

**Deliverables:**

- `sdks/go/client.go` -- gRPC client with connection management, retry logic
- `sdks/go/sensor.go` -- Sensor abstraction with event buffering (128 events, 128MB limit per spec)
- `sdks/go/rules.go` -- Rule cache with ETAG-based sync, background refresh
- `sdks/go/failure.go` -- Three failure modes: fail-safe (default), fail-closed, fail-open-with-cache
- `sdks/go/evaluate.go` -- Client-side rule evaluation (imports internal/rules)
- `sdks/go/options.go` -- Functional options for configuration
- Integration tests using SDK against sensor-api

**Key Design Points:**

- Imports `internal/rules` directly (same Go module, no server dependencies)
- Pre-compiled rules from server, SDK only evaluates
- Explicit `Flush()` required per ephemeral sensor principle
- Rule caching enables network resilience

### Phase 8: Web UI Foundation

**Rationale:** Required for rule management, user administration, and operational visibility. Specs define tk-web-ui as second service in two-service model.

**Deliverables:**

- `cmd/trapperkeeper/cmd/web_ui.go` -- Cobra subcommand for HTTP service
- `internal/core/web/server.go` -- net/http server with middleware chain
- `internal/core/web/middleware/` -- Session, CSRF, timezone, auth middlewares
- `internal/core/web/templates/` -- html/template with auto-escaping
- `internal/core/web/static/` -- Embedded static assets
- `internal/core/web/handlers/auth.go` -- Login, logout, password change
- Health endpoints: /healthz (liveness), /readyz (database ping)

**Key Design Points:**

- Server-side rendering with html/template (no JavaScript frameworks)
- Cookie-based sessions with 24h sliding expiry, database-backed storage
- CSRF protection via double-submit cookie pattern
- Timezone detection via browser Intl API, stored in cookie
- Bcrypt password hashing (cost factor 12)

### Phase 9: Management APIs

**Rationale:** CRUD operations for operational control. Web UI foundation must exist first.

**Deliverables:**

- `internal/core/web/handlers/rules.go` -- Rule list, create, edit, enable/disable, view history
- `internal/core/web/handlers/users.go` -- User list, create, edit, password reset
- `internal/core/web/handlers/apikeys.go` -- API key list, create, revoke (plaintext shown once)
- `internal/core/web/handlers/dashboard.go` -- Overview: rule count, event rates, error summary
- Template pages for all CRUD operations
- Form validation with inline error display

**Key Design Points:**

- Rules use append-only versioning (every edit creates new rule_id)
- API key secret shown exactly once at creation
- Emergency pause flag (in-memory, returns ETAG "PAUSED")
- No staged rollouts per simplicity principle

### Phase 10: Python SDK

**Rationale:** Python is dominant in data engineering. Pandas/NumPy vectorization enables high-throughput batch processing.

**Deliverables:**

- `sdks/python/trapperkeeper/client.py` -- gRPC client wrapper
- `sdks/python/trapperkeeper/sensor.py` -- Sensor with event buffering
- `sdks/python/trapperkeeper/pandas.py` -- DataFrame integration with vectorized evaluation
- `sdks/python/trapperkeeper/failure.py` -- Failure mode configuration
- `sdks/python/setup.py` -- PyPI packaging
- `sdks/python/tests/` -- Unit and integration tests

**Key Design Points:**

- Pure Python + generated protobuf (no pybind++ initially, can add later for performance)
- Vectorized operations on DataFrame columns for batch processing
- Same buffer limits: 128 events, 1MB per event, 128MB total
- Auto-flush on buffer limits to prevent memory exhaustion

### Phase 11: Java SDK

**Rationale:** Enterprise integration and Spark support for large-scale data processing.

**Deliverables:**

- `sdks/java/src/main/java/trapperkeeper/Client.java` -- gRPC client
- `sdks/java/src/main/java/trapperkeeper/Sensor.java` -- Sensor abstraction
- `sdks/java/src/main/java/trapperkeeper/spark/` -- Spark DataFrame integration
- `sdks/java/build.gradle` -- Gradle build with protobuf plugin
- Maven Central publishing configuration

**Key Design Points:**

- Standard Java gRPC patterns
- Spark integration for distributed evaluation
- JNI wrapper possible future optimization (not MVP)

### Phase 12: Integration Tests and Hardening

**Rationale:** Validate end-to-end flows and performance targets before production. Specs emphasize integration-first testing.

**Deliverables:**

- `tests/integration/sdk_test.go` -- Full SDK -> gRPC -> Database flows
- `tests/integration/webui_test.go` -- Web UI form submissions and sessions
- `tests/property/rules_test.go` -- Property-based tests for rule evaluation invariants
- `tests/load/benchmark_test.go` -- Sub-millisecond per-event validation
- `tests/migration/rollback_test.go` -- Migration up/down cycle verification
- `tests/tls/` -- TLS configuration and certificate validation tests
- CI pipeline integration

**Key Design Points:**

- Property-based tests over example-based per 01-principles
- Performance target: sub-millisecond per-event (local, excluding network)
- Test real database (SQLite for CI, PostgreSQL for staging)
- Test failure modes: network errors, invalid rules, buffer overflow

## Dependency Graph

```
Phase 5 (Rule Evaluation + Tests)
    |
    v
Phase 6 (Observability)
    |
    v
Phase 7 (Go SDK)
    |
    +---> Phase 8 (Web UI Foundation) --> Phase 9 (Management APIs)
    |
    +---> Phase 10 (Python SDK)
    |
    +---> Phase 11 (Java SDK)
    |
    v
Phase 12 (Integration Tests)
```

**Critical Path:** 5 -> 6 -> 7 -> 8 -> 9 -> 12

**Parallel Opportunities:**

- Phases 8, 10, and 11 can proceed in parallel after Phase 7
- Phase 9 can start once Phase 8 handlers are stubbed

## Estimated Scope

| Phase | New Files | Lines of Code (est) | Tests (est)  |
| ----- | --------- | ------------------- | ------------ |
| 5     | 6-8       | 400-600             | 800-1200     |
| 6     | 4-6       | 400-500             | 200-300      |
| 7     | 8-10      | 800-1000            | 500-700      |
| 8     | 15-20     | 1200-1500           | 400-600      |
| 9     | 8-12      | 1000-1200           | 300-400      |
| 10    | 8-10      | 600-800             | 400-500      |
| 11    | 10-12     | 800-1000            | 400-500      |
| 12    | 10-15     | 1500-2000           | (tests only) |

Total remaining: ~6700-8600 lines of production code, ~3000-4200 lines of tests.
