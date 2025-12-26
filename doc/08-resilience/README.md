---
doc_type: hub
status: active
date_created: 2025-11-07
primary_category: error-handling
consolidated_spokes:
  - doc/08-resilience/error-taxonomy.md
  - doc/08-resilience/logging-standards.md
  - doc/08-resilience/monitoring-strategy.md
  - doc/08-resilience/failure-modes.md
tags:
  - error-handling
  - resilience
  - degradation
---

# Resilience Architecture

## Context

Error handling guidance was previously fragmented across four ADRs with inconsistent patterns and coverage gaps. Type coercion error handling appeared in three locations with 70% content overlap. The "Least Intrusive Principle" was documented in two locations with 60% overlap. Critical error categories lacked unified guidance: database errors (40% scenario coverage), protocol errors, circuit breaker patterns, logging standards, and monitoring thresholds.

This hub consolidates error handling strategy into a single comprehensive entry point establishing error taxonomy (6 categories), response patterns (fail vs degrade vs retry decision tree), logging standards (slog), and monitoring strategy with specific alert thresholds. It provides unified patterns and cross-references to domain-specific implementations.

## Decision

We will implement a **unified error handling strategy** providing a single comprehensive entry point for all error handling decisions. This hub document establishes error taxonomy, response patterns, logging standards, and monitoring strategy.

This document serves as the resilience hub providing strategic overview of TrapperKeeper's error handling and degradation strategies. It consolidates error handling from architectural principles (Least Intrusive principle), type coercion semantics, missing field policies, and failure modes into a cohesive strategy.

### Core Principle: Least Intrusive

**Definition**: Minimize disruption when errors occur. Prefer graceful degradation over hard failures where safe.

**Key Insight**: "Least Intrusive" does NOT mean "ignore errors". It means:

- Continue processing unaffected operations
- Provide clear error feedback to users
- Log comprehensively for debugging and monitoring
- Degrade gracefully when safe
- Fail fast when data integrity at risk

**Application Across Error Categories**:

- **Type coercion failures**: Treat as condition failed (NOT missing field), continue evaluation
- **Missing fields**: Apply `on_missing_field` policy (skip/fail/match), continue evaluation
- **Network errors**: Degrade service, log errors, continue processing other events
- **Database errors**: Fail fast (no retry per simplicity principle)
- **Validation errors**: Reject invalid input with structured feedback, continue processing other requests
- **Protocol errors**: Return appropriate error code, continue processing other requests

**Cross-References**:

- Architectural Principles: Complete Least Intrusive principle rationale (Principle 2)
- Failure Modes: Degradation strategies for network partitions

### Error Taxonomy (6 Categories)

The error taxonomy provides consistent classification across all error scenarios.

**Category 1: Network Errors** - Connection timeout, DNS failure, TLS handshake failure, gRPC UNAVAILABLE status

- **Behavior**: Degrade service, log ERROR, continue processing other operations
- **Recovery**: Automatic retry with exponential backoff at network layer. Sensors degrade according to configured failure mode.
- **Monitoring**: Alert on sustained failures (>10 consecutive errors to same endpoint)

**Category 2: Type Coercion Errors** - String "abc" cannot coerce to number, null vs undefined ambiguity, array provided where scalar expected

- **Behavior**: Treat as condition failed (NOT missing field), continue evaluation of remaining conditions
- **Logging**: DEBUG level
- **Monitoring**: No alerts (expected behavior in schema-agnostic system). Track coercion rates per rule for debugging.

**Category 3: Missing Field Errors** - Field path not found in event data, nested object does not exist, array index out of bounds

- **Behavior**: Apply `on_missing_field` policy (skip/fail/match per rule configuration), continue evaluation
- **Logging**: DEBUG (skip policy) or WARN (fail policy)
- **Monitoring**: Track missing field rates per rule to detect schema drift. Alert if rate exceeds 50%.

**Category 4: Database Errors** - Connection failure, constraint violation, transaction rollback, disk full, permission denied

- **Behavior**: Fail fast (no retry per simplicity principle). Return 500 Internal Server Error to client.
- **Logging**: ERROR level
- **Monitoring**: Alert on ANY database error (critical system failure requiring immediate operator intervention)

**Category 5: Protocol Errors** - gRPC error codes, HTTP status codes, serialization failures, malformed requests, authentication failures

- **Behavior**: Return appropriate error code, log at WARN level, continue processing other requests
- **Logging**: WARN level
- **Monitoring**: Track error rates by code (4xx vs 5xx). Alert on >10% 5xx rate.

**Category 6: Validation Errors** - Rule expression syntax error, invalid UUID format, resource limit exceeded, operator/field_type mismatch

- **Behavior**: Reject at API boundary with structured error response, continue processing other requests
- **Logging**: INFO level
- **Monitoring**: Track validation error rates by field. Alert if specific field error rate >20%.

**Cross-References**:

- Type System: Type coercion failure semantics
- Schema Evolution: Missing field handling
- Validation Hub: Validation error handling
- Failure Modes: Network error degradation strategies

### Error Response Pattern Matrix

| Category      | Behavior                    | Logging Level | Monitoring                    | HTTP Status    | gRPC Code            |
| ------------- | --------------------------- | ------------- | ----------------------------- | -------------- | -------------------- |
| Network       | Degrade, retry              | ERROR         | Alert >10 consecutive         | N/A (internal) | UNAVAILABLE (14)     |
| Type Coercion | Condition failed            | DEBUG         | Track rate                    | N/A (internal) | N/A                  |
| Missing Field | Apply policy                | DEBUG/WARN    | Track rate, alert >50%        | N/A (internal) | N/A                  |
| Database      | Fail fast                   | ERROR         | Alert ALL                     | 500            | INTERNAL (13)        |
| Protocol      | Return code, continue       | WARN          | Track by code, alert >10% 5xx | 400-599        | Varies by scenario   |
| Validation    | Reject, structured response | INFO          | Track by field, alert >20%    | 422/400        | INVALID_ARGUMENT (3) |

**Cross-References**:

- API Service: gRPC error codes
- Web Framework: HTTP status codes

### Decision Tree: When to Fail vs Degrade vs Retry

Follow this decision tree for all error scenarios.

```
1. Does error affect data integrity?
   ├─ YES → Fail fast
   │         Examples: Database errors, migration failures, constraint violations
   │         Action: Return 500, log ERROR, alert operator
   └─ NO → Continue to question 2

2. Can operation succeed later without changes?
   ├─ YES → Retry with backoff
   │         Examples: Network timeouts, transient connection failures
   │         Action: Exponential backoff, log ERROR, alert on sustained failures
   │         Failure Mode Selection: Apply configured sensor failure_mode
   │           ├─ fail-safe → Use cached rules until TTL, then disable all rules
   │           ├─ fail-closed → Raise exception after retry exhaustion
   │           └─ fail-open-with-cache → Use cached rules indefinitely
   └─ NO → Continue to question 3

3. Can system operate with reduced functionality?
   ├─ YES → Degrade gracefully
   │         Examples: Rule evaluation continues for other events, network service degraded
   │         Action: Use cached data, log ERROR/WARN, continue processing
   │         Network Degradation: Apply failure_mode configuration
   │           ├─ fail-safe → Operate as no-op pass-through (default, least intrusive)
   │           ├─ fail-closed → Halt processing (strict compliance mode)
   │           └─ fail-open-with-cache → Use stale rules (intermittent connectivity)
   └─ NO → Continue to question 4

4. Is error caused by client input?
   ├─ YES → Reject with structured error
   │         Examples: Validation errors, malformed requests, authentication failures
   │         Action: Return 422/400/401, log INFO/WARN, provide clear feedback
   └─ NO → Continue to question 5

5. Is error expected in schema-agnostic system?
   ├─ YES → Log at DEBUG, continue
   │         Examples: Type coercion failures, missing fields with skip policy
   │         Action: Log DEBUG, continue evaluation, track metrics
   └─ NO → Log at ERROR/WARN based on severity
           Examples: Unexpected conditions, edge cases
           Action: Log comprehensively for debugging
```

**Cross-References**:

- Failure Modes: Complete failure mode matrix and configuration patterns

### Logging Standards (slog)

Use Go's `log/slog` standard library exclusively for all logging (NEVER use `fmt.Println` or legacy `log` package).

**Log Levels and Usage**:

- **ERROR**: System-level failures requiring operator intervention (database errors, service crashes)
- **WARN**: Degraded functionality or potential issues (network failures before alert threshold)
- **INFO**: Normal operational events (service startup/shutdown, validation errors)
- **DEBUG**: Detailed diagnostic information (type coercion failures, missing fields with skip policy)

**Structured Logging Format**:

```go
// ERROR example
slog.Error("Database query failed",
    slog.String("target", "database"),
    slog.String("error", err.Error()),
    slog.String("query", queryDescription),
    slog.String("operation", operationType),
)

// DEBUG example
slog.Debug("Type coercion failed, treating as condition failed",
    slog.String("target", "rule_evaluation"),
    slog.String("rule_id", rule.ID),
    slog.String("field", fieldPath),
    slog.String("value_type", actualType),
    slog.String("expected_type", expectedType),
)
```

**Log Sanitization (CRITICAL SECURITY)**:

- **NEVER log**: Passwords, API secrets, session tokens, HMAC keys, TLS private keys
- **Sanitize SQL**: Log parameterized queries, NOT interpolated values
- **Sanitize user input**: Escape or truncate to prevent log injection

**Cross-References**:

- Failure Modes: Complete logging standards with target taxonomy

### Monitoring and Alerting Strategy

**Metrics to Track** (expose via Prometheus endpoint):

1. **Error rates by category**: `errors_total{category="network|database|protocol|validation", severity="error|warn|info"}`
2. **Error rates by endpoint**: `http_errors_total{endpoint="/api/rules", status_code="422"}`
3. **Database error count**: `database_errors_total{operation="query|insert|update|delete"}`
4. **Network failure streak**: `network_consecutive_failures{endpoint="https://api.example.com"}`
5. **Validation error rates by field**: `validation_errors_total{field="conditions[0].operator", error_type="invalid_value"}`

**Alerting Thresholds**:

| Metric             | Threshold                           | Severity | Action                                          |
| ------------------ | ----------------------------------- | -------- | ----------------------------------------------- |
| Database errors    | ANY error                           | CRITICAL | Page on-call, check database health immediately |
| Network failures   | >10 consecutive to same endpoint    | HIGH     | Check endpoint availability, review logs        |
| Protocol 5xx rate  | >10% over 5 minutes                 | HIGH     | Investigate server errors, check resource usage |
| Validation errors  | >20% for specific field over 1 hour | MEDIUM   | Review UI clarity, update error messages        |
| Missing field rate | >50% for rule over 1 hour           | MEDIUM   | Check schema drift, notify rule owner           |

**Cross-References**:

- Operational Endpoints: Health check and metrics exposure

## Consequences

**Benefits**:

- Single comprehensive entry point for all error handling decisions eliminates need to consult 4 ADRs
- Error taxonomy covering 6 categories with 100% scenario coverage (previously 40-70% coverage)
- Unified decision tree for fail vs degrade vs retry eliminates inconsistent choices
- Structured logging standards with slog enable comprehensive debugging
- Monitoring strategy with specific alert thresholds enables proactive incident response
- Category-specific guidance with cross-references preserves detailed implementation in spoke documents

**Trade-offs**:

- Hub-and-spoke structure requires maintaining cross-references across 5 documents
- Developers must read this hub first, then consult spoke documents for domain-specific details
- Monitoring infrastructure (Prometheus, alerting) adds operational complexity
- Structured logging requires discipline to maintain consistent field names and targets

## Related Documents

**Consolidated Spokes** (this hub consolidates):

- [error-taxonomy.md](error-taxonomy.md): Complete error category specifications (6 categories: network, type coercion, missing field, database, protocol, validation) with behavior patterns, logging requirements, and recovery strategies
- [logging-standards.md](logging-standards.md): Structured logging patterns using slog with log levels, required fields by target, distributed tracing, and log sanitization security requirements
- [monitoring-strategy.md](monitoring-strategy.md): Prometheus metrics specifications, alert thresholds by error category, alert destinations, and incident response procedures
- [failure-modes.md](failure-modes.md): Network error degradation strategies, failure mode configurations (fail-safe, fail-closed, fail-open-with-cache), rule caching strategy, and implementation examples

**Dependencies** (foundational documents):

- Architectural Principles: Least Intrusive principle (Principle 2), Simplicity principle (Principle 4)
- API Service: gRPC error codes and protocol specifications
- Web Framework: HTTP status codes and error response formats

**References** (related hubs/documents):

- Type System: Type coercion failure semantics consolidated in error-taxonomy.md Category 2
- Schema Evolution: Missing field handling consolidated in error-taxonomy.md Category 3
- Validation Hub: Validation error handling consolidated in error-taxonomy.md Category 6

**Extended by**:

- Database Migrations: Migration failure handling uses fail-fast pattern from error-taxonomy.md Category 4
- Batch Processing: Batch-specific error handling follows coercion semantics from error-taxonomy.md Category 2
