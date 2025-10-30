# ADR-024: Testing Strategy

## Revision log

| Date | Description |
|------|-------------|
| 2025-10-29 | Document created |

## Context

TrapperKeeper faces unique testing challenges that make traditional test pyramid approaches impractical:

**Ephemeral workloads**: Sensors live for minutes to hours (Airflow/Spark job duration), making traditional persistent test environments misaligned with production reality.

**Schema diversity**: Users deploy many distinct dataset types per installation. and TrapperKeeper has no control over these formats.

**Startup constraints**: Small team cannot maintain extensive test pyramids with thousands of unit tests. Testing strategy must maximize confidence while minimizing maintenance burden.

**Schema-agnostic architecture**: Server has zero schema understanding (ADR-001), so comprehensive unit testing of schema validation is impossible. The system must work correctly regardless of data shape.

**High-throughput bursts**: Production sees irregular traffic patterns—30-minute periods of silence followed by 10,000 simultaneous Parquet file arrivals. Tests must validate batch processing behavior under load.

Traditional test pyramids assume:
- Stable, pre-registered schemas (not true here)
- Persistent infrastructure to test against (ephemeral sensors violate this)
- Large teams with dedicated QA resources (five engineers total)
- Low diversity in data shapes (15+ formats contradict this)

These assumptions do not hold for TrapperKeeper. We need a testing approach optimized for schema diversity, ephemeral infrastructure, and lean team constraints.

## Decision

We will adopt an **Integration-First Testing** strategy implementing ADR-001 Principle #6, organized around the Testing Trophy model rather than the traditional pyramid.

### 1. Test Hierarchy - Testing Trophy Model

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

**Static Analysis (widest base)**:
- Linting (`clippy`, `pylint`)
- Type checking (`mypy` for Python)
- Security scanning (`cargo-audit`, `bandit`)
- Fast, cheap, catches common errors before runtime

**Integration Tests (large middle - PRIMARY STRATEGY)**:
- Run against containerized `tk-sensor-api` service
- Tests use real SDKs to communicate via gRPC
- Validate full business flows: auth → rule sync → evaluation → event storage → database query
- Each test starts fresh Docker environment
- Majority of test effort focuses here

**Unit Tests (narrow top)**:
- Reserved for complex business logic with multiple branches
- Type coercion edge cases (ADR-016)
- Performance-critical path optimizations
- Mathematical functions (sampling probabilities, HMAC generation)
- NOT used for testing simple getters, DTOs, or straight-line code

**E2E Tests**:
- SDK integration tests ARE the E2E tests
- Rust SDK (reference implementation): Comprehensive sensor-api testing (partial batch failures, DLQ behavior, rule syncing)
- Python/JS SDKs: Validate SDK-specific behavior end-to-end
- Web UI: Server-side HTML validation with metadata tags (`data-testid`)

### 2. Test Environment

All integration tests run in ephemeral Docker containers (see Appendix A for configuration example).

**Key principles**:
- Each test suite gets isolated fresh state
- No persistent staging environment (aligns with ephemeral sensor principle)
- Database runs in-memory tmpfs for speed
- Tests clean up containers after execution
- CI/CD runs tests in parallel with cached Docker layers

### 3. SDK Testing Boundaries

**Rust SDK** (reference implementation):
- Tests `sensor-api` service thoroughly
- Validates partial batch failures (some records succeed, some fail)
- Tests DLQ behavior when events fail submission
- Validates rule syncing with ETAG caching
- Tests authentication failures and retry logic
- Validates fail-safe mode when API unreachable (ADR-021)

**Python SDK**:
- Tests Python-specific data type handling (datetime, Decimal, numpy arrays)
- Validates Pandas DataFrame integration (ADR-023)
- Tests UTF-8 encoding from Python's internal UTF-32 representation
- End-to-end: Data submitted via Python SDK appears correctly in database

**Future JavaScript SDK**:
- Tests JavaScript-specific type coercion (Date objects, BigInt)
- Validates browser vs. Node.js environment differences
- End-to-end: Data submitted via JS SDK appears correctly in database

**Web UI Testing**:
- Server-side HTML validation (not JavaScript browser testing)
- Use `data-testid` metadata tags for test hooks
- Integration tests POST to UI endpoints, validate redirects and database state
- No Selenium/Playwright (ADR-001: MVP Simplicity)

### 4. Property-Based Testing

**Default approach for schema variations**:

Property-based testing generates arbitrary data to explore edge cases automatically. This is the primary strategy for handling schema diversity.

**Tools**:
- **Python**: `hypothesis` library
- **Rust**: `proptest` for property-based testing with shrinking support

**Conceptual example** (see Appendix C for implementation):

Property: Rule evaluation never crashes, regardless of data shape.

This single test validates millions of data variations:
- Nested objects of arbitrary depth
- Mixed types in arrays
- Unicode strings with emoji/control chars
- Null values at any level
- Empty objects and arrays

**Seed-based reproducibility**:
- Property tests use fixed seeds for reproducible failures
- CI/CD logs seed values on failure
- Developers reproduce exact failure locally with seed

**When to use property-based tests**:
- Field path resolution with wildcards (ADR-015)
- Type coercion edge cases (ADR-016)
- Schema evolution scenarios (ADR-017)
- Batch processing with mixed data types (ADR-023)

**When NOT to use property-based tests**:
- Business logic requiring specific data (authentication, API key rotation)
- Tests requiring deterministic output (event ID generation)
- Tests validating specific error messages

### 5. What NOT to Test

**Excluded from testing scope** (reduces maintenance burden):

1. **gRPC protocol implementation details**:
   - Do not test gRPC serialization/deserialization
   - Do not test HTTP/2 framing or multiplexing
   - Trust that `grpc-go` library handles protocol correctly

2. **API contract changes**:
   - Schema is volatile in greenfield development
   - Breaking changes expected during MVP iteration
   - No contract testing (Pact, OpenAPI validation) until API stabilizes

3. **Implementation internals**:
   - Do not test private functions or internal data structures
   - Do not test intermediate states (only final outcomes)
   - Do not test for specific log messages (observability, not correctness)

4. **Edge cases without business impact**:
   - Do not test UUIDv7 generation edge cases (trust library)
   - Do not test UTF-8 encoding edge cases (trust standard library)
   - Do not test database driver internals (trust `sqlx`)

5. **Performance regressions** (until optimization phase):
   - No benchmark tests in CI/CD during MVP
   - Manual benchmarking for known bottlenecks only
   - Defer comprehensive performance testing to post-MVP

**Focus testing on high-value business flows** (see section 8).

### 6. Test Data Management

**Property-based generation (default)**:
- Generate arbitrary JSON-serializable data using `hypothesis` or `proptest`
- Use fixed seeds for reproducible test runs
- Covers massive range of data shapes with minimal code

**Static test data (when required)**:
- Authentication: Specific usernames, API keys, HMAC signatures
- Rule examples: Hand-crafted datasets demonstrating specific conditions
- Regression tests: Exact data that triggered past bugs

**Test data location**:
```
tests/
  fixtures/
    events/
      iot_waveform.parquet       # 500K-point sample from real customer
      compressed_json.json.gz    # Vendor-specific compressed format
      mqtt_csv_batch.csv         # Time-series batch from MQTT stream
    rules/
      complex_dnf_example.json   # Rule with 10+ clauses for perf testing
```

**Data generation helpers**:
```rust
fn generate_iot_record(
    num_sensors: usize,
    include_waveform: bool,
    compress: bool
) -> serde_json::Value {
    // Generate realistic IoT record matching customer data patterns
}
```

### 7. Mocking Guidelines

**Mocking allowed ONLY for difficult-to-replicate failure scenarios**.

**Do NOT mock** (use real Docker containers instead):
- API offline/unreachable (use `docker-compose down`)
- Database connection failure (use `docker-compose pause db`)
- Slow network (use `tc` traffic control in container)
- Invalid API credentials (use wrong key in test)

**DO mock** (when unavoidable):
- Clock/time for UUIDv7 generation edge cases (time going backwards)
- External vendor APIs (if TrapperKeeper integrates with third-party services)
- Specific gRPC error codes from server (e.g., `RESOURCE_EXHAUSTED` for rate limiting)
- NTP synchronization failures (clock drift >1 hour)

**Mocking tools**:
- **Rust**: `mockall` for trait mocking or `mockito` for HTTP mocking
- **Python**: `unittest.mock` or `pytest-mock`
- **Benchmarking**: `criterion` for statistical performance testing

Example use case (see Appendix C for implementation): Testing UUIDv7 generation when client clock is ahead of server.

### 8. High-Value Test Definition

Focus integration tests on these business-critical flows:

**1. User Authentication and Authorization**:
- Cookie-based Web UI login (ADR-011)
- HMAC-based API authentication (ADR-012)
- API key generation and rotation
- Multi-tenant isolation (even though single-tenant in MVP)

**2. Rule Syncing and Evaluation**:
- ETAG-based rule sync (ADR-005)
- Rule state transitions: draft → active → disabled (ADR-018)
- DNF evaluation correctness (ADR-014)
- Field path resolution with wildcards (ADR-015)
- Type coercion (ADR-016)
- Missing field handling with `on_missing_field` modes (ADR-017)

**3. Event Data Submission and Processing**:
- Successful event submission
- Partial batch failures (some records fail, some succeed)
- DLQ behavior for failed events
- Event deduplication (if implemented)
- Metadata namespace validation (`$tk.*` prefix, ADR-020)

**4. API Key Generation and HMAC Rotation**:
- Generate API key with correct HMAC signature
- Rotate HMAC secret without breaking existing keys
- Reject invalid HMAC signatures
- Handle clock drift in HMAC timestamp validation

**5. Data Appears Correctly in Database**:
- Events stored with correct UUIDv7 IDs (ADR-003)
- JSONL format correct (ADR-019)
- Full audit trail (record snapshot, rule snapshot, matched clauses)
- Query events by rule ID, time range, sensor ID

**Tests validating these flows provide 80% confidence with 20% effort.**

### 9. CI/CD Integration

**Test execution in continuous integration** (see Appendix B for workflow example):

**Parallel execution**:
- Matrix strategy runs component tests in parallel
- Each matrix job gets isolated Docker environment
- Total test time: ~5-10 minutes (not 30+ minutes for sequential)

**Failure debugging**:
- All tests include `trace_id` in logs for request tracing
- Docker logs collected on test failure
- Failed property tests log seed value for reproduction

## Consequences

### Benefits

1. **High confidence in system behavior**:
   - Integration tests validate actual business flows, not mocked interactions
   - Real gRPC communication catches serialization bugs, authentication issues, network failures
   - Database queries validate data correctness end-to-end

2. **Low maintenance burden**:
   - Fewer tests to maintain than traditional pyramid (100 integration tests vs. 1000 unit tests)
   - Tests aligned with business value, not implementation details
   - No brittle mocks that break on refactoring

3. **Natural schema diversity handling**:
   - Property-based tests explore millions of data shapes automatically
   - Single test validates arbitrary JSON structures
   - Regression tests capture specific edge cases from production

4. **Tests validate actual business flows**:
   - "Can user authenticate and submit events?" is one integration test
   - Traditional approach: 10+ unit tests mocking each layer
   - Integration test provides higher confidence with less code

5. **Alignment with architectural principles**:
   - Ephemeral test environments match ephemeral sensor design (ADR-001)
   - Schema-agnostic tests match schema-agnostic architecture (ADR-001)
   - Fail-safe testing validates degradation modes (ADR-021)

6. **Fast iteration velocity**:
   - Five-engineer team spends less time writing/maintaining tests
   - More time building features, less time debugging test infrastructure
   - Tests fail less often due to implementation refactoring

### Tradeoffs

1. **Slower feedback loop than unit tests**:
   - Integration test: 2-5 seconds (Docker startup + gRPC call + database query)
   - Unit test: 10-50ms (in-memory only)
   - **Mitigation**: Parallel test execution, Docker layer caching, in-memory databases (tmpfs)
   - **Acceptable**: 5-minute test suite better than 30-minute test suite with 1000 unit tests

2. **Debugging complexity**:
   - Integration test failure: "Is it SDK, gRPC, server, or database?"
   - Unit test failure: "This function has a bug"
   - **Mitigation**: Include `trace_id` in all logs, collect Docker logs on failure, use debugger attach to containers
   - **Acceptable**: Rare failures worth debugging vs. constant test maintenance

3. **Higher compute requirements**:
   - Each test needs Docker containers (CPU, memory, disk)
   - CI/CD runners need more resources than unit-test-only pipelines
   - **Mitigation**: GitHub Actions has generous free tier, self-hosted runners for scale
   - **Acceptable**: $50/month CI cost is cheaper than engineer time maintaining unit tests

4. **Test data management**:
   - Property-based tests generate huge datasets (GBs for waveform tests)
   - Need to manage test data lifecycle (cleanup, storage)
   - **Mitigation**: tmpfs databases, automatic Docker volume cleanup, property test size limits
   - **Acceptable**: Tradeoff for comprehensive schema coverage

5. **Initial test setup complexity**:
   - docker-compose.test.yml requires maintenance (versions, configuration)
   - Developers need Docker installed locally
   - **Mitigation**: Documentation, `make test` wrapper, CI/CD validates setup
   - **Acceptable**: One-time setup cost, not recurring maintenance

## Implementation

### Phase 1: Foundation (Week 1-2)

1. **Create `docker-compose.test.yml`**:
   - Define `sensor-api`, `web-ui`, `db` services
   - Use tmpfs for PostgreSQL data directory (in-memory speed)
   - Expose gRPC (50051) and HTTP (8080) ports

2. **Set up Rust SDK integration tests**:
   - Test: Authenticate → Sync rules → Submit events → Query database
   - Test: Partial batch failure handling
   - Test: DLQ behavior
   - Test: Fail-safe mode when API unreachable

3. **Add property-based test framework**:
   - Rust: Add `proptest` dependency
   - Python: Add `hypothesis` dependency
   - Create helper functions for JSON generation

4. **CI/CD integration**:
   - GitHub Actions workflow with matrix strategy
   - Docker layer caching
   - Log collection on failure

### Phase 2: Expand Coverage (Week 3-4)

5. **Add Python SDK integration tests**:
   - Pandas DataFrame submission (ADR-023)
   - Python type handling (datetime, Decimal)
   - UTF-8 encoding validation

6. **Add Web UI integration tests**:
   - Cookie-based authentication
   - Rule CRUD operations
   - Event query UI

7. **Property-based test coverage**:
   - Field path resolution with arbitrary nesting
   - Type coercion edge cases
   - Wildcard evaluation with empty arrays

### Phase 3: Optimization (Week 5+)

8. **Test performance tuning**:
   - Benchmark test suite execution time
   - Optimize Docker layer caching
   - Add parallel test execution within test suites

9. **Developer experience**:
   - `make test` command runs full integration suite
   - `make test-fast` runs static analysis + unit tests only
   - Documentation for debugging failed tests

## Appendix A: Test Environment Configuration

**Docker Compose structure for integration tests**:

```yaml
# docker-compose.test.yml
version: '3.8'
services:
  sensor-api:
    image: trapperkeeper/sensor-api:test
    environment:
      - DATABASE_URL=postgres://test@db:5432/trapperkeeper_test
      - AUTH_SECRET=test-secret-key
    depends_on:
      - db
    ports:
      - "50051:50051"  # gRPC

  db:
    image: postgres:16-alpine
    environment:
      - POSTGRES_USER=test
      - POSTGRES_PASSWORD=test
      - POSTGRES_DB=trapperkeeper_test
    tmpfs:
      - /var/lib/postgresql/data  # In-memory for speed

  web-ui:
    image: trapperkeeper/web-ui:test
    environment:
      - DATABASE_URL=postgres://test@db:5432/trapperkeeper_test
      - SESSION_SECRET=test-session-key
    depends_on:
      - db
    ports:
      - "8080:8080"  # HTTP
```

This configuration provides isolated, ephemeral test environments with in-memory databases for maximum speed.

## Appendix B: CI/CD Integration

**GitHub Actions workflow for continuous integration**:

```yaml
# .github/workflows/test.yml
name: Integration Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        component: [sensor-api, web-ui, go-sdk, python-sdk]

    steps:
      - uses: actions/checkout@v3

      - name: Cache Docker layers
        uses: actions/cache@v3
        with:
          path: /tmp/.buildx-cache
          key: ${{ runner.os }}-docker-${{ hashFiles('**/Dockerfile') }}

      - name: Start test environment
        run: docker-compose -f docker-compose.test.yml up -d

      - name: Run integration tests
        run: |
          docker-compose -f docker-compose.test.yml \
            run --rm ${{ matrix.component }}-tests

      - name: Collect logs on failure
        if: failure()
        run: docker-compose -f docker-compose.test.yml logs

      - name: Cleanup
        if: always()
        run: docker-compose -f docker-compose.test.yml down -v
```

This workflow runs component tests in parallel with isolated Docker environments per matrix job.

## Appendix C: Example Test Implementations

### Environment Setup

Integration tests initialize Docker environment and establish database connections before executing test scenarios.

### Integration Tests

**Full-flow event submission test**:

```rust
// tests/integration/sensor_api_test.rs
use sqlx::PgPool;
use trapperkeeper_sdk::Sensor;
use serde_json::json;

#[tokio::test]
async fn test_event_submission_full_flow() -> Result<(), Box<dyn std::error::Error>> {
    // Setup: Start docker-compose environment (handled by test framework)
    let api_url = "http://localhost:50051";
    let db_pool = connect_to_test_db().await?;

    // Create API key via Web UI
    let api_key = create_api_key_via_web_ui("test-sensor").await?;

    // Initialize SDK
    let sensor = Sensor::new(api_url, &api_key).await?;

    // Create rule via Web UI
    let rule = Rule {
        name: "Detect high temperature".to_string(),
        expression: "$.temperature > 80".to_string(),
        severity: "critical".to_string(),
    };
    let rule_id = create_rule_via_web_ui(&rule).await?;

    // Sync rules to sensor
    sensor.sync_rules().await?;

    // Submit event matching rule
    let event = json!({
        "sensor_id": "temp-sensor-01",
        "temperature": 95.5,
        "timestamp": "2025-10-29T10:00:00Z",
    });
    let event_id = sensor.submit_event(event).await?;
    assert!(!event_id.is_empty());

    // Validate event in database
    let stored_event = sqlx::query_as::<_, StoredEvent>(
        "SELECT id, severity, matched_rule_id FROM events WHERE id = $1"
    )
    .bind(&event_id)
    .fetch_one(&db_pool)
    .await?;

    assert_eq!(event_id, stored_event.id);
    assert_eq!("critical", stored_event.severity);
    assert_eq!(rule_id, stored_event.matched_rule_id);

    Ok(())
}
```

### Property-Based Tests

**Python hypothesis example for arbitrary JSON evaluation**:

```python
from hypothesis import given
import hypothesis.strategies as st

@given(st.recursive(
    st.none() | st.booleans() | st.floats() | st.text(),
    lambda children: st.lists(children) | st.dictionaries(st.text(), children)
))
def test_rule_evaluation_handles_arbitrary_json(data):
    """
    Property: Rule evaluation never crashes, regardless of data shape.

    This single test validates millions of data variations:
    - Nested objects of arbitrary depth
    - Mixed types in arrays
    - Unicode strings with emoji/control chars
    - Null values at any level
    - Empty objects and arrays
    """
    sensor = Sensor(api_key="test-key")

    # Rule: $.users[*].age > 18
    # Expected: Evaluates without crashing, handles missing fields gracefully
    result = sensor.evaluate_record(data)

    # Postcondition: Either matched, skipped, or failed gracefully
    assert result in ['matched', 'skipped', 'type_error']
    assert sensor.is_alive()  # Did not crash
```

**Wildcard field path resolution test**:

```python
# tests/property/test_field_paths.py
from hypothesis import given, strategies as st
import trapperkeeper.sdk as tk

@given(st.recursive(
    st.none() | st.booleans() | st.integers() | st.floats() | st.text(),
    lambda children: st.lists(children) | st.dictionaries(st.text(), children),
    max_leaves=100,
))
def test_wildcard_resolution_never_crashes(data):
    """
    Property: Wildcard field path resolution handles arbitrary data shapes.

    Tests millions of variations:
    - Nested objects of arbitrary depth
    - Arrays of mixed types
    - Missing keys at any level
    - Null values
    """
    sensor = tk.Sensor(api_key="test-key", mode="dry-run")

    # Rule with wildcard: $.users[*].profile.age
    sensor.add_rule(rule_id="test-rule", expression="$.users[*].profile.age > 18")

    # Evaluate arbitrary data
    result = sensor.evaluate(data)

    # Postcondition: Evaluation completes without exception
    assert result.status in ['matched', 'not_matched', 'skipped']
    assert sensor.is_healthy()
```

### Mocking

**Legitimate mock for clock drift scenario**:

```rust
// Testing UUIDv7 generation when client clock is ahead of server
use mockall::predicate::*;
use mockall::mock;

mock! {
    Clock {}
    impl Clock {
        fn now(&self) -> SystemTime;
    }
}

let mut mock_clock = MockClock::new();
mock_clock.expect_now()
    .returning(|| SystemTime::now() + Duration::from_secs(7200)); // 2 hours ahead

let sensor = Sensor::with_clock(api_key, mock_clock);
let event_id = sensor.generate_event_id();

// Verify: System warns but does not reject
assert!(logs.contains("clock drift detected"));
assert!(event_id.is_some());
```

## Related Decisions

This ADR implements ADR-001 Principle #6 (Integration-First Testing) and defines testing boundaries for components described in other ADRs.

**Implements**:
- **ADR-001: Architectural Principles** - Implements Principle #6 (Integration-First Testing)

**Depends on**:
- **ADR-002: SDK Model** - Defines SDK behavior tested by integration tests
- **ADR-003: UUID Strategy** - UUIDv7 generation tested for correctness
- **ADR-004: Database Backend** - Multi-database support requires test coverage
- **ADR-005: API Service Architecture** - gRPC protocol tested by integration tests

**Constrains**:
- **ADR-014: Rule Expression Language** - DNF evaluation correctness validated by tests
- **ADR-015: Field Path Resolution** - Wildcard resolution tested with property-based tests
- **ADR-016: Type System and Coercion** - Type coercion edge cases tested with property-based tests
- **ADR-017: Schema Evolution** - Missing field handling tested with property-based tests
- **ADR-018: Rule Lifecycle** - Rule state transitions tested by integration tests
- **ADR-019: Event Schema and Storage** - JSONL format and UUIDv7 validated by database queries
- **ADR-021: Failure Modes and Degradation** - Fail-safe mode tested by stopping Docker containers
- **ADR-023: Batch Processing and Vectorization** - Pandas/Spark integration tested end-to-end

## Future Considerations

### 1. Contract Testing (Post-MVP)

When API stabilizes, add contract testing:
- **Pact** for consumer-driven contracts
- Validate SDK expectations match server implementation
- Run against multiple server versions (backwards compatibility)

**Trigger**: After 3+ months of API stability (no breaking changes)

### 2. Performance Regression Testing

Add benchmark tests when optimization phase begins:
- Baseline: 1M events/minute throughput
- Detect >10% regressions in CI/CD
- Use Go benchmarking framework (`testing.B`)

**Trigger**: After MVP launch, when customers report performance issues

### 3. Chaos Engineering

Introduce deliberate failures to test resilience:
- Random container kills during test execution
- Network partitions between services
- Database connection pool exhaustion

**Trigger**: When scaling beyond single customer (multi-tenant deployment)

### 4. Visual Regression Testing (Web UI)

Add screenshot-based testing for UI:
- Percy, Chromatic, or BackstopJS
- Detect unintended CSS changes

**Trigger**: When Web UI becomes customer-facing (not just internal tooling)

### 5. Load Testing

Simulate production-scale traffic:
- 10,000 simultaneous sensor connections
- 1M events/minute sustained throughput
- Use `k6` or `locust` for load generation

**Trigger**: When onboarding first high-volume customer (>100K events/day)

This testing strategy prioritizes **pragmatism over perfection**. Integration-first approach provides high confidence with low maintenance, aligning with the five-engineer startup constraint and TrapperKeeper's ephemeral, schema-agnostic architecture.
