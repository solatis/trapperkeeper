---
doc_type: spoke
status: active
primary_category: error-handling
hub_document: doc/08-resilience/README.md
tags:
  - error-handling
  - taxonomy
  - error-categories
---

# Error Taxonomy

## Context

TrapperKeeper's error handling strategy requires consistent error classification across all system layers (SDK, API, Web UI, rule engine, database). Without a unified taxonomy, engineers must make ad-hoc decisions about logging levels, monitoring thresholds, and recovery strategies, leading to inconsistent error handling patterns.

This document provides the complete error taxonomy with 6 categories, each specifying behavior patterns, logging requirements, monitoring thresholds, and recovery strategies. This taxonomy ensures consistent error handling across all TrapperKeeper components.

**Hub Document**: This document is part of the Resilience Architecture hub. See [README.md](README.md) for strategic overview of error handling principles, decision trees, and relationships to other resilience components.

## Category 1: Network Errors

**Definition**: Failures in network communication between system components or external services.

**Examples**:

- Connection timeout (client cannot reach server within timeout period)
- DNS failure (hostname resolution fails)
- TLS handshake failure (certificate validation fails, protocol mismatch)
- gRPC UNAVAILABLE status (service unreachable or temporarily down)
- HTTP connection refused (server not listening on port)
- Network partition (loss of connectivity between services)

**Behavior**: Degrade service, log ERROR, continue processing other operations

Network errors are transient by nature and should not halt unrelated operations. The system degrades gracefully according to configured failure mode while continuing to process operations that don't depend on the failed network connection.

**Logging**:

```go
slog.Error("Network connection failed",
    slog.String("target", "network"),
    slog.String("endpoint", endpointURL),
    slog.String("error", err.Error()),
    slog.Int("retry_attempt", attempt),
)
```

**Required fields**: `target: "network"`, `endpoint`, `error`, `retry_attempt`

**Monitoring**: Alert on sustained failures (>10 consecutive errors to same endpoint)

Single network failures are expected and transient. Only sustained failure patterns indicate a systemic issue requiring operator intervention.

**Recovery**: Automatic retry with exponential backoff at network layer. Sensors degrade according to configured failure mode.

### Failure Mode Configurations

**Configuration scope**: Set at sensor initialization time, applies to all network-related failures (sync failures, event POST failures, etc.)

**Available modes**:

| Mode                   | Behavior                                                | Use Case                                                                    |
| ---------------------- | ------------------------------------------------------- | --------------------------------------------------------------------------- |
| `fail-safe` (default)  | Disable all rules when offline, operate as pass-through | Production pipelines, analytics jobs, non-critical monitoring               |
| `fail-closed`          | Raise exception when offline, halt processing           | Compliance-critical data (GDPR, HIPAA), PII filtering, regulatory reporting |
| `fail-open-with-cache` | Use cached rules indefinitely when offline              | Edge deployments, intermittent connectivity, mobile sensors                 |

**Configuration syntax** (Python SDK example):

```python
sensor = Sensor(
    api_key="...",
    tags=["production"],
    failure_mode="fail-safe"  # Default
)
```

**Failure mode behavior matrix**:

| Scenario                       | fail-safe (default)             | fail-closed           | fail-open-with-cache     |
| ------------------------------ | ------------------------------- | --------------------- | ------------------------ |
| API unreachable at startup     | Operate as no-op, retry         | Exception, halt       | Operate as no-op, retry  |
| API unreachable during sync    | Use cache until TTL, then no-op | Exception, halt       | Use cache indefinitely   |
| Cache expired, API unreachable | Empty rule set (no-op)          | Exception, halt       | Use stale cache          |
| Event POST failure             | Log warning, continue           | Log warning, continue | Log warning, continue    |
| Network partition (5+ minutes) | Pass-through mode               | Pipeline halted       | Operate with stale rules |

**Rationale**: Per-sensor configuration allows flexibility for different pipeline requirements without per-rule complexity. Fail-safe default protects most users while enabling strict mode when needed.

### Rule Caching Strategy

**Cache implementation**:

- In-memory only (no disk cache)
- Lives with sensor object
- Destroyed on sensor exit
- No persistent storage across process restarts

**Cache expiration**:

- Default TTL: 5 minutes after last successful sync
- When cache expires and API unreachable: fall back to configured failure mode
- Cache refreshed on every successful sync

**Ephemeral Sensor Architecture Benefits**:

In-memory cache suffices for ephemeral sensors because:

1. **Short Sensor Lifespan**: Sensors live minutes to hours—TTL-based expiration works because sensor lifecycle is shorter than typical network partition duration
2. **No Persistent Identity**: Absence of persistent identity means no split-brain scenarios where multiple instances claim the same sensor identity with different cached rule sets
3. **Automatic Cleanup**: Sensors destroyed at job end provide automatic cache cleanup—no stale cache management infrastructure required
4. **Bounded Memory**: Short lifecycle means bounded cache growth—no risk of unbounded accumulation over long-running processes

**Rationale**: In-memory cache avoids file locking conflicts with multiple sensors on same host. Simpler implementation with no disk I/O. Ephemeral architecture makes in-memory cache both sufficient and optimal.

**Cross-Reference**: See [failure-modes.md](failure-modes.md) for additional degradation details and implementation examples.

## Category 2: Type Coercion Errors

**Definition**: Failures when attempting to convert event data values to expected types during rule evaluation.

**Examples**:

- String "abc" cannot coerce to number
- null vs undefined ambiguity (language-specific edge cases)
- Array provided where scalar expected
- Object provided where primitive expected
- Numeric string "123" vs actual number 123 coercion
- Boolean coercion edge cases (truthy/falsy values)

**Behavior**: Treat as **condition failed** (NOT missing field), continue evaluation of remaining conditions

This is a critical semantic distinction. Type coercion failure means the field exists but has an incompatible type. The condition evaluates to false, but other conditions in the rule continue evaluation.

**Logging**:

```go
slog.Debug("Type coercion failed, treating as condition failed",
    slog.String("target", "rule_evaluation"),
    slog.String("rule_id", rule.ID),
    slog.String("field", fieldPath),
    slog.String("value_type", actualType),
    slog.String("expected_type", expectedType),
)
```

**Required fields**: `target: "rule_evaluation"`, `rule_id`, `field`, `value_type`, `expected_type`

**Monitoring**: No alerts (expected behavior in schema-agnostic system). Track coercion rates per rule for debugging.

Type coercion failures are normal in a schema-agnostic system where event schemas vary. High coercion rates may indicate schema drift but are not error conditions.

**Recovery**: Not applicable (semantic behavior, not an error condition)

**CRITICAL SEMANTIC NOTE**: Type coercion failure ≠ missing field. This distinction affects rule evaluation logic:

- **Type coercion failure**: Field exists, wrong type → Condition evaluates to false
- **Missing field**: Field absent → Apply `on_missing_field` policy (skip/fail/match)

**Cross-References**:

- Type System: Complete type coercion rules and semantics
- Schema Evolution: Missing vs failed coercion disambiguation

## Category 3: Missing Field Errors

**Definition**: Failures when field paths referenced in rule conditions cannot be resolved in event data.

**Examples**:

- Field path not found in event data (`user.email` when `user` object missing)
- Nested object does not exist (`payment.card.number` when `payment.card` is null)
- Array index out of bounds (`items[5]` when array has 3 elements)
- Wildcard expansion produces empty result set (`*.id` when no matching fields)

**Behavior**: Apply `on_missing_field` policy (skip/fail/match per rule configuration), continue evaluation

The `on_missing_field` policy provides per-rule control over missing field semantics, enabling different strategies for optional vs required fields.

**Logging**:

```go
// When policy is "skip" (default)
slog.Debug("Field not found, skipping condition per policy",
    slog.String("target", "rule_evaluation"),
    slog.String("rule_id", rule.ID),
    slog.String("field", fieldPath),
    slog.String("policy", "skip"),
)

// When policy is "fail"
slog.Warn("Field not found, marking rule as failed per policy",
    slog.String("target", "rule_evaluation"),
    slog.String("rule_id", rule.ID),
    slog.String("field", fieldPath),
    slog.String("policy", "fail"),
)
```

**Required fields**: `target: "rule_evaluation"`, `rule_id`, `field`, `policy`

**Monitoring**: Track missing field rates per rule to detect potential schema drift. Alert if rate exceeds 50% (indicates likely schema change).

High missing field rates suggest event schemas have changed in ways that affect rule effectiveness. This is not an error but requires investigation.

**Recovery**: Configure `on_missing_field` policy per rule based on requirements. Use `fail` for critical validation, `skip` for optional fields, `match` when missing field should trigger action.

**Policy Semantics**:

| Policy           | Behavior                                       | Use Case                                     |
| ---------------- | ---------------------------------------------- | -------------------------------------------- |
| `skip` (default) | Ignore condition, continue evaluation          | Optional fields, schema evolution resilience |
| `fail`           | Treat as condition failed (rule doesn't match) | Required fields, strict validation           |
| `match`          | Treat as condition succeeded (missing = match) | Absence detection, negative assertions       |

**Cross-Reference**: Schema Evolution document provides complete `on_missing_field` policy specification and configuration examples.

## Category 4: Database Errors

**Definition**: Failures in database operations including queries, transactions, migrations, and connection management.

**Examples**:

- Connection failure (database unreachable, connection pool exhausted)
- Constraint violation (unique key, foreign key, check constraint)
- Transaction rollback (deadlock, timeout, explicit rollback)
- Disk full (write fails due to storage exhaustion)
- Permission denied (insufficient database privileges)
- Query timeout (long-running query exceeds threshold)

**Behavior**: **Fail fast** (no retry per simplicity principle). Return 500 Internal Server Error to client.

Database errors indicate system-level problems requiring immediate attention. Retrying database operations can mask underlying issues (disk full, network partition) and worsen system state through connection pool exhaustion.

**Logging**:

```go
slog.Error("Database operation failed",
    slog.String("target", "database"),
    slog.String("error", err.Error()),
    slog.String("query", queryDescription), // NOT actual query with values
    slog.String("operation", operationType),
)
```

**Required fields**: `target: "database"`, `error`, `query_description` (sanitized), `operation_type`

**CRITICAL SECURITY NOTE**: NEVER log actual SQL queries with interpolated values. Always log parameterized queries with placeholders.

**Monitoring**: Alert on **ANY** database error (critical system failure requiring immediate operator intervention)

Database errors are never expected in normal operation. Any database error indicates a systemic problem requiring immediate investigation.

**Recovery**: Operator intervention required. Check database health, disk space, permissions, connection pool exhaustion. No automatic retry to avoid cascading failures.

**Rationale**: Database errors indicate system-level problems requiring immediate attention. Retrying database operations can mask underlying issues (disk full, network partition) and worsen system state through connection pool exhaustion.

**Cross-References**:

- Architectural Principles: Simplicity principle (Principle 4) - no retry logic
- Database Migrations: Migration failure handling using fail-fast pattern

## Category 5: Protocol Errors

**Definition**: Failures in protocol-level communication including HTTP status codes, gRPC error codes, serialization failures, and malformed requests.

**Examples**:

- gRPC error codes (INVALID_ARGUMENT, UNAUTHENTICATED, PERMISSION_DENIED)
- HTTP status codes (400, 401, 403, 404, 500)
- Serialization failures (JSON parse error, protobuf decode error)
- Malformed requests (missing required fields, invalid content-type)
- Authentication failures (missing credentials, invalid token)
- Request timeout (client timeout before response sent)

**Behavior**: Return appropriate error code, log at WARN level, continue processing other requests

Protocol errors are per-request failures that should not affect other requests. The system returns appropriate error codes to clients while continuing to process other operations.

**Logging**:

```go
slog.Warn("Protocol error in request processing",
    slog.String("target", "protocol"),
    slog.String("request_id", requestID),
    slog.Any("client_info", clientMetadata),
    slog.Int("error_code", statusCode),
    slog.String("error", err.Error()),
)
```

**Required fields**: `target: "protocol"`, `request_id`, `error_code`, `error`

**Monitoring**: Track error rates by code (4xx vs 5xx). Alert on >10% 5xx rate (indicates server-side issues).

4xx errors indicate client bugs (authentication, validation, malformed requests). 5xx errors indicate server problems requiring investigation.

**Recovery**: Client retries with corrected request. 4xx errors are client bugs, 5xx errors may be transient.

### gRPC Error Codes

| Code                | Value | HTTP Equivalent | Description                                             |
| ------------------- | ----- | --------------- | ------------------------------------------------------- |
| `INVALID_ARGUMENT`  | 3     | 400             | Malformed request (bad syntax, missing required fields) |
| `UNAUTHENTICATED`   | 16    | 401             | Missing or invalid credentials                          |
| `PERMISSION_DENIED` | 7     | 403             | Insufficient permissions                                |
| `NOT_FOUND`         | 5     | 404             | Resource not found                                      |
| `INTERNAL`          | 13    | 500             | Server error (database errors, unexpected panics)       |
| `UNAVAILABLE`       | 14    | 503             | Service unavailable (see Category 1: Network Errors)    |

### HTTP Status Codes

| Code | Description           | Use Case                                                                 |
| ---- | --------------------- | ------------------------------------------------------------------------ |
| 400  | Bad Request           | Malformed request (bad syntax, unparseable JSON)                         |
| 401  | Unauthorized          | Authentication required (missing or invalid credentials)                 |
| 403  | Forbidden             | Authenticated but insufficient permissions                               |
| 404  | Not Found             | Resource not found                                                       |
| 422  | Unprocessable Entity  | Validation error (well-formed but semantically invalid) - See Category 6 |
| 500  | Internal Server Error | Database errors, unexpected panics                                       |
| 503  | Service Unavailable   | Service temporarily unavailable (see Category 1)                         |

**Cross-References**:

- API Service: Complete gRPC service specification and error codes
- Web Framework: HTTP service and status code semantics

## Category 6: Validation Errors

**Definition**: Failures when user input fails validation rules at API boundaries (UI, API, runtime).

**Examples**:

- Rule expression syntax error (invalid operator, malformed expression)
- Invalid UUID format (malformed UUID string)
- Resource limit exceeded (rule count, condition count, field path length)
- Operator/field_type mismatch (numeric operator on string field)
- Missing required fields (rule without name, condition without operator)
- Value out of range (negative TTL, excessive string length)

**Behavior**: Reject at API boundary with structured error response, continue processing other requests

Validation errors indicate client bugs, not system failures. The request is rejected with detailed error feedback while other requests continue processing.

**Logging**:

```go
slog.Info("Validation failed for user input",
    slog.String("target", "validation"),
    slog.String("request_id", requestID),
    slog.String("field", fieldName),
    slog.String("validation_error", errorType),
)
```

**Required fields**: `target: "validation"`, `request_id`, `field`, `validation_error`

**Monitoring**: Track validation error rates by field. Alert if specific field error rate >20% (indicates potential client bug or unclear UI).

High validation error rates for specific fields suggest UX problems (unclear error messages, confusing UI) or client bugs requiring investigation.

**Recovery**: Client corrects input and resubmits. Validation errors are client bugs, not system failures.

### Structured Error Response

Validation errors return structured JSON with detailed error information:

```json
{
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "Invalid rule expression",
    "field": "conditions[0].operator",
    "details": "Operator 'invalid_op' not supported. Supported: eq, ne, gt, gte, lt, lte, contains, starts_with, ends_with, regex, in",
    "request_id": "01936a3e-1234-7b3c-9d5e-abcdef123456"
  }
}
```

**Required fields**:

- `code`: Machine-readable error code (e.g., `VALIDATION_ERROR`, `INVALID_UUID`)
- `message`: Human-readable summary
- `field`: Specific field that failed validation (JSON path notation)
- `details`: Detailed explanation with guidance for correction
- `request_id`: Request identifier for tracing and debugging

### HTTP Status Codes

- **422 Unprocessable Entity**: Semantic validation error (well-formed but invalid data)
- **400 Bad Request**: Syntactic validation error (malformed request, cannot parse)

### gRPC Error Code

- **INVALID_ARGUMENT (3)**: Used for all validation errors in gRPC API

**Cross-Reference**: Validation Hub provides complete validation strategy and input sanitization specifications across all layers (UI, API, runtime, database).

## Error Category Summary Matrix

| Category      | Affects Data Integrity? | Retry Strategy      | Alerting Threshold       | Log Level  | Client Recovery                       |
| ------------- | ----------------------- | ------------------- | ------------------------ | ---------- | ------------------------------------- |
| Network       | No                      | Exponential backoff | >10 consecutive failures | ERROR      | Automatic (SDK handles)               |
| Type Coercion | No                      | N/A (semantic)      | None (track metrics)     | DEBUG      | N/A (rule evaluation)                 |
| Missing Field | No                      | N/A (semantic)      | >50% missing rate        | DEBUG/WARN | N/A (rule evaluation)                 |
| Database      | Yes                     | None (fail fast)    | ANY error                | ERROR      | Retry request (transient errors only) |
| Protocol      | No                      | Client retries      | >10% 5xx rate            | WARN       | Correct request, retry                |
| Validation    | No                      | None (reject)       | >20% for specific field  | INFO       | Correct input, resubmit               |

## Edge Cases and Limitations

**Network Error Edge Cases**:

- **Partial network failure**: Some endpoints reachable, others not → Each endpoint tracks failure independently
- **DNS resolution intermittent**: Retry at DNS level before treating as connection failure
- **TLS certificate expired**: Treated as configuration error, not transient network failure

**Type Coercion Edge Cases**:

- **Numeric string vs number**: "123" (string) vs 123 (number) → Coercion succeeds for numeric operators
- **null vs undefined**: Language-specific semantics (JavaScript vs Python) → Treat both as missing value
- **Array vs scalar**: Array [42] cannot coerce to number → Coercion fails, condition evaluates false

**Missing Field Edge Cases**:

- **Wildcard expansion empty**: `*.id` matches no fields → Treat as missing field, apply policy
- **Null vs missing**: Field exists with null value vs field absent → Treat null as present (not missing)
- **Array out of bounds**: `items[10]` when array length is 5 → Treat as missing field

**Database Error Edge Cases**:

- **Transient connection failure**: Network issue vs database crash → Fail fast regardless (no retry)
- **Constraint violation on insert**: Unique key violation → Log as ERROR, return 500 (indicates race condition)
- **Query timeout**: Long-running query exceeds threshold → Log as ERROR, investigate query performance

**Protocol Error Edge Cases**:

- **gRPC UNAVAILABLE**: Network issue (Category 1) or service overload → Classify as Network Error
- **HTTP 503**: Service unavailable vs maintenance mode → Classify as Network Error
- **Authentication vs Authorization**: 401 (missing credentials) vs 403 (insufficient permissions) → Use correct code

**Validation Error Edge Cases**:

- **Malformed JSON**: Cannot parse request → Return 400 (syntactic error)
- **Valid JSON, invalid semantics**: Well-formed but violates rules → Return 422 (semantic error)
- **Missing required field**: Field absent vs field empty → Both treated as validation error

## Related Documents

**Hub Document**:

- [README.md](README.md): Strategic overview of resilience architecture, decision trees, and Least Intrusive principle

**Related Spokes** (siblings in resilience hub):

- [logging-standards.md](logging-standards.md): Structured logging patterns for each error category
- [monitoring-strategy.md](monitoring-strategy.md): Alert thresholds and metrics for each error category
- [failure-modes.md](failure-modes.md): Network degradation strategies and failure mode implementations

**Dependencies** (foundational documents):

- Architectural Principles: Least Intrusive principle (Principle 2), Simplicity principle (Principle 4)
- API Service: gRPC error codes and protocol specifications
- Web Framework: HTTP status codes and error response formats

**Extended by**:

- Type System: Detailed type coercion rules for Category 2
- Schema Evolution: Missing field policy specifications for Category 3
- Validation Hub: Complete validation specifications for Category 6
- Database Migrations: Migration-specific error handling using Category 4 patterns
