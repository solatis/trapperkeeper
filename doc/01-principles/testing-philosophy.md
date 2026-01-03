---
doc_type: spoke
status: active
primary_category: testing
hub_document: doc/01-principles/README.md
tags:
  - testing-trophy
  - integration-first
  - property-based
---

# Testing Philosophy

## Context

TrapperKeeper faces unique testing challenges that make traditional test pyramid approaches impractical. Ephemeral workloads (sensors that live for minutes to hours matching Airflow/Spark job duration), schema diversity (users deploy many distinct dataset types per installation with no control over formats), and startup constraints (five-engineer team with internal/ package architecture cannot maintain extensive test pyramids) require a different testing philosophy.

Traditional test pyramids assume stable pre-registered schemas, persistent infrastructure, large teams with dedicated QA resources, and low diversity in data shapes. These assumptions do not hold for TrapperKeeper.

**Hub Document**: This document is part of the Principles Hub. See [Principles Overview](README.md) for the comprehensive architectural foundation that establishes Integration-First Testing as core principle #6.

## Integration-First Testing Strategy

We adopt an **Integration-First Testing** strategy, organized around the Testing Trophy model rather than the traditional pyramid.

This approach prioritizes integration tests as the primary testing strategy, with static analysis as the foundation and unit tests reserved for complex logic only. The Testing Trophy model better aligns with TrapperKeeper's architectural realities: schema-agnostic design, ephemeral sensors, and small team constraints.

**Rationale**: Integration tests validate actual business flows (auth → rule sync → evaluation → storage → query) rather than mocked interactions. This provides higher confidence with fewer tests to maintain, naturally handles schema diversity, and aligns with ephemeral sensor architecture.

**Cross-References**:

- Testing Integration Patterns: Docker fixtures, database seeding, API testing
- Testing Examples: Concrete implementations and common scenarios
- Principles Architecture Section 6: Integration-First Testing principle

## Testing Trophy Model

The Testing Trophy inverts traditional pyramid thinking:

```
              ╱──────────────╲
             ╱   Unit Tests   ╲     ← Narrow top: complex logic only
            ╱──────────────────╲
           ╱                    ╲
          ╱  Integration Tests   ╲   ← Large middle: majority of tests
         ╱                        ╲
        ╱──────────────────────────╲
       ╱     Static Analysis        ╲  ← Widest base: cheap, fast
      ╱──────────────────────────────╲
```

### Static Analysis (Widest Base)

Fast, cheap, catches common errors before runtime:

- Linting: `golangci-lint` for Go, `pylint` for Python
- Type checking: `mypy` for Python SDKs
- Security scanning: `gosec`, `bandit`
- Zero runtime cost, runs in CI before any tests execute

### Integration Tests (Large Middle - Primary Strategy)

Majority of test effort focuses here:

- Run against containerized `tk-sensor-api` service
- Tests use real SDKs to communicate via gRPC
- Validate full business flows end-to-end
- Each test starts fresh Docker environment
- Examples: auth → sync → evaluate → store → query

**Benefits**: Validates actual behavior without mocks, catches integration issues early, naturally handles schema diversity, provides high confidence with low maintenance.

### Unit Tests (Narrow Top)

Reserved for complex business logic only:

- Type coercion edge cases (Type System and Coercion)
- Performance-critical path optimizations
- Mathematical functions (sampling probabilities, HMAC generation)
- NOT used for simple getters, DTOs, or straight-line code

**Rationale**: Unit tests have value for complex branches with multiple code paths. Straight-line code is better validated by integration tests that exercise real behavior.

## Test Environment Design

All integration tests run in ephemeral Docker containers matching ephemeral sensor architecture.

**Key Principles**:

- Each test suite gets isolated fresh state
- No persistent staging environment
- Database runs in-memory tmpfs for speed
- Tests clean up containers after execution
- CI/CD runs tests in parallel with cached Docker layers

**Example**:

```yaml
# docker-compose.test.yml
services:
  sensor-api:
    image: trapperkeeper/sensor-api:test
    environment:
      - DATABASE_URL=postgres://test@db:5432/trapperkeeper_test
    depends_on:
      - db
    ports:
      - "50051:50051" # gRPC

  db:
    image: postgres:16-alpine
    tmpfs:
      - /var/lib/postgresql/data # In-memory for speed
```

**Cross-References**:

- Testing Integration Patterns: Complete Docker configuration
- Testing Examples Section 1: Environment setup code

## SDK Testing Boundaries

Clear boundaries define what each SDK tests to avoid duplication.

### Go SDK (Reference Implementation)

Tests `sensor-api` service thoroughly as reference:

- Validates partial batch failures (some records succeed, some fail)
- Tests DLQ behavior when events fail submission
- Validates rule syncing with ETAG caching
- Tests authentication failures and retry logic
- Validates fail-safe mode when API unreachable

### Language-Specific SDKs

Each SDK tests language-specific concerns only:

**Python SDK**:

- Python-specific data type handling (datetime, Decimal, numpy arrays)
- Pandas DataFrame integration
- UTF-8 encoding from Python's internal UTF-32 representation
- End-to-end: Data submitted via Python SDK appears correctly in database

**Future JavaScript SDK**:

- JavaScript-specific type coercion (Date objects, BigInt)
- Browser vs. Node.js environment differences
- End-to-end: Data submitted via JS SDK appears correctly in database

**Rationale**: Go SDK validates server behavior comprehensively. Language SDKs validate language bindings and type conversions only. Avoids testing server logic N times.

## Property-Based Testing for Schema Diversity

Property-based testing is the **default approach for schema variations**.

Property-based testing generates arbitrary data to explore edge cases automatically. This is the primary strategy for handling TrapperKeeper's schema diversity challenge.

**Tools**:

- **Go**: `gopter` for property-based testing with shrinking support
- **Python**: `hypothesis` library

**Conceptual Example**:

Property: Rule evaluation never crashes, regardless of data shape.

This single test validates millions of data variations:

- Nested objects of arbitrary depth
- Mixed types in arrays
- Unicode strings with emoji/control chars
- Null values at any level
- Empty objects and arrays

**When to Use**:

- Field path resolution with wildcards
- Type coercion edge cases
- Schema evolution scenarios
- Batch processing with mixed data types

**When NOT to Use**:

- Business logic requiring specific data (authentication, API key rotation)
- Tests requiring deterministic output (event ID generation)
- Tests validating specific error messages

**Seed-Based Reproducibility**: Property tests use fixed seeds for reproducible failures. CI/CD logs seed values on failure enabling developers to reproduce exact failure locally.

**Cross-References**:

- Testing Examples Section 3: Property-based test implementations
- Field Path Resolution: Wildcard resolution testing
- Type System and Coercion: Type coercion edge case testing

## What NOT to Test

Excluded from testing scope to reduce maintenance burden:

**1. gRPC Protocol Implementation Details**:

- Do not test gRPC serialization/deserialization
- Do not test HTTP/2 framing or multiplexing
- Trust that `google.golang.org/grpc` library handles protocol correctly

**2. API Contract Changes**:

- Schema is volatile in greenfield development
- Breaking changes expected during MVP iteration
- No contract testing (Pact, OpenAPI validation) until API stabilizes

**3. Implementation Internals**:

- Do not test private functions or internal data structures
- Do not test intermediate states (only final outcomes)
- Do not test for specific log messages (observability, not correctness)

**4. Edge Cases Without Business Impact**:

- Do not test UUIDv7 generation edge cases (trust library)
- Do not test UTF-8 encoding edge cases (trust standard library)
- Do not test database driver internals (trust `github.com/jackc/pgx`)

**5. Performance Regressions** (until optimization phase):

- No benchmark tests in CI/CD during MVP
- Manual benchmarking for known bottlenecks only
- Defer comprehensive performance testing to post-MVP

**Rationale**: Focus testing on high-value business flows. Library implementations are tested by their maintainers. Performance optimization deferred until performance requirements are defined.

## High-Value Test Definition

Focus integration tests on business-critical flows that provide maximum confidence:

**1. User Authentication and Authorization**:

- Cookie-based Web UI login
- HMAC-based API authentication
- API key generation and rotation
- Multi-tenant isolation (even though single-tenant in MVP)

**2. Rule Syncing and Evaluation**:

- ETAG-based rule sync
- Rule state transitions: draft → active → disabled
- DNF evaluation correctness
- Field path resolution with wildcards
- Type coercion
- Missing field handling with `on_missing_field` modes

**3. Event Data Submission and Processing**:

- Successful event submission
- Partial batch failures (some records fail, some succeed)
- DLQ behavior for failed events
- Event deduplication (if implemented)
- Metadata namespace validation (`$tk.*` prefix)

**4. API Key Generation and HMAC Rotation**:

- Generate API key with correct HMAC signature
- Rotate HMAC secret without breaking existing keys
- Reject invalid HMAC signatures
- Handle clock drift in HMAC timestamp validation

**5. Data Appears Correctly in Database**:

- Events stored with correct UUIDv7 IDs
- JSONL format correct
- Full audit trail (record snapshot, rule snapshot, matched clauses)
- Query events by rule ID, time range, sensor ID

**Impact**: Tests validating these flows provide 80% confidence with 20% effort.

**Cross-References**:

- Testing Integration Patterns: Implementation patterns for these flows
- Testing Examples: Concrete test implementations

## Mocking Guidelines

Mocking allowed ONLY for difficult-to-replicate failure scenarios.

**Do NOT Mock** (use real Docker containers instead):

- API offline/unreachable (use `docker-compose down`)
- Database connection failure (use `docker-compose pause db`)
- Slow network (use `tc` traffic control in container)
- Invalid API credentials (use wrong key in test)

**DO Mock** (when unavoidable):

- Clock/time for UUIDv7 generation edge cases (time going backwards)
- External vendor APIs (if TrapperKeeper integrates with third-party services)
- Specific gRPC error codes from server (e.g., `RESOURCE_EXHAUSTED` for rate limiting)
- NTP synchronization failures (clock drift >1 hour)

**Mocking Tools**:

- **Go**: Manual interface mocks (Go idiom) or `httptest` for HTTP mocking
- **Python**: `unittest.mock` or `pytest-mock`
- **Benchmarking**: `testing.B` for statistical performance testing

**Rationale**: Real containers provide higher fidelity tests than mocks. Mocks introduce maintenance burden when implementation details change. Reserve mocks for scenarios impossible to replicate with real infrastructure.

**Cross-References**:

- Testing Examples Section 4: Mock example for clock drift scenario

## Related Documents

**Dependencies** (read these first):

- Principles Architecture: Establishes Integration-First Testing as principle #6

**Related Spokes** (siblings in this hub):

- Testing Integration Patterns: Docker fixtures, database seeding, API testing strategies
- Testing Examples: Comprehensive examples, common scenarios, anti-patterns

**Extended by**:

- Web Framework Selection: Specifies net/http test utilities and goquery for HTML validation
- Client/Server Package Separation: Defines internal/ package testing boundaries
