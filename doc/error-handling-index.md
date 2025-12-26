---
doc_type: index
status: active
date_created: 2025-11-07
date_updated: 2025-11-07
primary_category: error-handling
cross_cutting:
  - error-handling
maintainer: Resilience Team
last_review: 2025-11-07
next_review: 2026-02-07
---

# Error Handling Index

## Purpose

This index provides navigation to all documentation addressing **error handling** across the Trapperkeeper system. Use this as a discovery mechanism for error taxonomy, failure modes, resilience patterns, and recovery strategies regardless of their primary domain. Error handling is critical for system reliability, graceful degradation, and operational stability.

## Quick Reference

| Category                  | Description                                                                               | Key Documents                                                                                |
| ------------------------- | ----------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| Error Taxonomy            | 6 error categories: network, type coercion, missing field, database, protocol, validation | [Error Taxonomy](08-resilience/error-taxonomy.md), [Resilience Hub](08-resilience/README.md) |
| Failure Modes             | Fail-safe, fail-closed, fail-open-with-cache configurations                               | [Failure Modes](08-resilience/failure-modes.md)                                              |
| Response Patterns         | Fail vs degrade vs retry decision tree                                                    | [Resilience Hub](08-resilience/README.md) Section 3                                          |
| Least Intrusive Principle | System degrades to pass-through rather than failing pipelines                             | [Architectural Principles](01-principles/README.md) Section 2                                |
| Logging Standards         | Error logging with severity levels and structured context                                 | [Logging Standards](08-resilience/logging-standards.md)                                      |

## Core Concepts

### Error Taxonomy (6 Categories)

TrapperKeeper classifies errors into 6 categories, each with specific handling patterns: Category 1 (Network Errors), Category 2 (Type Coercion Errors), Category 3 (Missing Field Errors), Category 4 (Database Errors), Category 5 (Protocol Errors), Category 6 (Validation Errors). Each category has specific retry strategies, logging requirements, and recovery mechanisms.

**Relevant Documentation:**

- **[Error Taxonomy](08-resilience/error-taxonomy.md)** - Complete error category specifications → See Section 1 for detailed category definitions
- **[Resilience Hub](08-resilience/README.md)** - Strategic overview of error handling architecture → See Section 1 for error taxonomy introduction
- **[Logging Standards](08-resilience/logging-standards.md)** - Error category logging patterns → See Section 5 for error event structure

### Category 1: Network Errors

Network errors occur when sensors cannot communicate with the API server (connection refused, timeout, DNS failure). Default behavior: fail-safe mode (disable all rules, become no-op pass-through). Configurable alternatives: fail-closed (block all data) or fail-open-with-cache (use cached rules with TTL). Network errors are transient; retry with exponential backoff is appropriate.

**Relevant Documentation:**

- **[Failure Modes](08-resilience/failure-modes.md)** - Complete failure mode configurations → See Section 2 for fail-safe/fail-closed/fail-open-with-cache implementations
- **[Error Taxonomy](08-resilience/error-taxonomy.md)** - Network error handling details → See Category 1 for retry strategies
- **[Resilience Hub](08-resilience/README.md)** - Network error overview → See Section 4 for network failure scenarios

### Category 2: Type Coercion Errors

Type coercion errors occur when field values cannot be coerced to expected types for operator evaluation (string "abc" coerced to integer). Default behavior: treat as condition failed, continue evaluation. Type coercion errors are persistent; retry is not appropriate. Logging at WARN level enables rule debugging.

**Relevant Documentation:**

- **[Error Taxonomy](08-resilience/error-taxonomy.md)** - Type coercion error handling → See Category 2 for coercion failure semantics
- **[Type System and Coercion](04-rule-engine/type-system-coercion.md)** - Coercion rules and failure modes → See Section 2 for safe coercion matrix
- **[Resilience Hub](08-resilience/README.md)** - Type coercion error overview → See Section 5 for coercion failure patterns

### Category 3: Missing Field Errors

Missing field errors occur when rule conditions reference fields not present in event data (field `x.y.z` does not exist). Configurable behavior via `on_missing_field`: `skip` (default, treat as condition failed), `fail` (treat as rule failed), `warn` (log warning and skip). Missing field errors are persistent; retry is not appropriate.

**Relevant Documentation:**

- **[Error Taxonomy](08-resilience/error-taxonomy.md)** - Missing field error handling → See Category 3 for `on_missing_field` semantics
- **[Schema Evolution](04-rule-engine/schema-evolution.md)** - How missing fields relate to schema changes → See Section 2 for missing field strategies
- **[Resilience Hub](08-resilience/README.md)** - Missing field error overview → See Section 6 for schema evolution patterns

### Category 4: Database Errors

Database errors occur during rule storage, event storage, or user authentication (connection pool exhausted, deadlock, constraint violation). Default behavior: return error to client, log at ERROR level, do not retry (client must retry). Transient database errors (connection pool exhausted) vs persistent errors (constraint violation) require different handling.

**Relevant Documentation:**

- **[Error Taxonomy](08-resilience/error-taxonomy.md)** - Database error handling → See Category 4 for transient vs persistent classification
- **[Database Backend](09-operations/database-backend.md)** - Database error scenarios → See Section 4 for error handling patterns
- **[Resilience Hub](08-resilience/README.md)** - Database error overview → See Section 7 for database failure modes

### Category 5: Protocol Errors

Protocol errors occur when clients send malformed requests (invalid gRPC message, missing required fields, wrong message type). Default behavior: return INVALID_ARGUMENT or BAD_REQUEST to client, log at WARN level, do not retry. Protocol errors are persistent; client must fix request before retry.

**Relevant Documentation:**

- **[Error Taxonomy](08-resilience/error-taxonomy.md)** - Protocol error handling → See Category 5 for protocol violation patterns
- **[API Service](02-architecture/api-service.md)** - Protocol error responses → See Section 5 for gRPC error codes
- **[Resilience Hub](08-resilience/README.md)** - Protocol error overview → See Section 8 for protocol failure handling

### Category 6: Validation Errors

Validation errors occur when input violates validation rules (password too short, rule expression syntax error, invalid field path). Default behavior: return validation error to client with structured error details, log at INFO level, do not retry. Validation errors are persistent; client must correct input before retry.

**Relevant Documentation:**

- **[Error Taxonomy](08-resilience/error-taxonomy.md)** - Validation error handling → See Category 6 for validation error structure
- **[Validation Hub](07-validation/README.md)** - Validation error patterns → See Section 7 for structured validation errors
- **[Resilience Hub](08-resilience/README.md)** - Validation error overview → See Section 9 for validation failure handling

### Failure Mode Configurations

TrapperKeeper supports three failure mode configurations for network partitions: fail-safe (default, disable all rules), fail-closed (block all data), fail-open-with-cache (use cached rules). Failure mode selection depends on use case: observability (fail-safe), security (fail-closed), high-availability (fail-open-with-cache).

**Relevant Documentation:**

- **[Failure Modes](08-resilience/failure-modes.md)** - Complete failure mode specifications → See Section 2 for configuration patterns
- **[Resilience Hub](08-resilience/README.md)** - Failure mode strategy → See Section 2 for mode selection guidance
- **[Architectural Principles](01-principles/README.md)** - Least Intrusive principle → See Section 2 for fail-safe rationale

### Fail vs Degrade vs Retry Decision Tree

TrapperKeeper uses a decision tree to classify error handling strategies: Fail (return error, log, do not retry) for persistent errors (validation, protocol, permanent database errors). Degrade (continue with reduced functionality) for missing fields and type coercion. Retry (exponential backoff) for transient errors (network, temporary database errors).

**Relevant Documentation:**

- **[Resilience Hub](08-resilience/README.md)** - Complete decision tree → See Section 3 for fail/degrade/retry classification
- **[Error Taxonomy](08-resilience/error-taxonomy.md)** - Decision tree application to each category → See Section 2 for per-category strategies
- **[Failure Modes](08-resilience/failure-modes.md)** - How failure modes relate to decision tree → See Section 3 for mode-specific decisions

### Least Intrusive by Default Principle

TrapperKeeper prioritizes graceful degradation over strict failure. System degrades to pass-through rather than failing pipelines. Network failures default to fail-safe (disable rules, become no-op). Missing fields default to skip (treat as condition failed). Type coercion failures treat as condition failed, continue evaluation. Least Intrusive principle ensures observability layer does not break production pipelines.

**Relevant Documentation:**

- **[Architectural Principles](01-principles/README.md)** - Complete Least Intrusive specification → See Section 2 for principle rationale
- **[Failure Modes](08-resilience/failure-modes.md)** - How failure modes implement Least Intrusive → See Section 1 for principle alignment
- **[Resilience Hub](08-resilience/README.md)** - Least Intrusive in error handling → See Section 10 for degradation patterns

### Rule Caching Strategy

Fail-open-with-cache mode uses in-memory rule cache with TTL (default 5 minutes) to enable rule evaluation during network partitions. Ephemeral sensors benefit: sensor initialization fetches rules, subsequent evaluations use in-memory cache. Cache invalidation on rule version change. Cache TTL prevents stale rule usage in long-running sensors.

**Relevant Documentation:**

- **[Failure Modes](08-resilience/failure-modes.md)** - Complete caching strategy → See Section 2.3 for fail-open-with-cache implementation
- **[Resilience Hub](08-resilience/README.md)** - Caching rationale → See Section 2 for cache architecture
- **[SDK Model](02-architecture/sdk-model.md)** - How ephemeral sensors benefit from caching → See Section 3 for cache integration

## Domain Coverage Matrix

| Domain         | Coverage | Key Document                                                                |
| -------------- | -------- | --------------------------------------------------------------------------- |
| Architecture   | ✓        | [SDK Model](02-architecture/sdk-model.md) (error propagation)               |
| API Design     | ✓        | [API Service](02-architecture/api-service.md) (gRPC error codes)            |
| Database       | ✓        | [Database Backend](09-operations/database-backend.md) (database errors)     |
| Security       | ✓        | [Security Hub](06-security/README.md) (security error handling)             |
| Performance    | ✓        | [Performance Hub](05-performance/README.md) (error handling overhead)       |
| Validation     | ✓        | [Validation Hub](07-validation/README.md) (validation errors)               |
| Configuration  | ✓        | [Configuration Management](09-operations/configuration.md) (config errors)  |
| Testing        | ✓        | [Testing Philosophy](01-principles/testing-philosophy.md) (error testing)   |
| Deployment     | ✓        | [Health Endpoints](09-operations/health-endpoints.md) (health check errors) |
| Error Handling | ✓        | [Resilience Hub](08-resilience/README.md)                                   |

## Patterns and Best Practices

### Fail-Safe Degradation Pattern

**Description**: Default behavior degrades to pass-through rather than failing pipelines. Network failures disable all rules, becoming no-op. Missing fields skip conditions rather than error. Type coercion failures treat as condition failed. Fail-safe ensures observability layer does not break production.

**Used In**:

- [Failure Modes](08-resilience/failure-modes.md) Section 2.1
- [Architectural Principles](01-principles/README.md) Section 2
- [Resilience Hub](08-resilience/README.md) Section 2

### Structured Error Reporting Pattern

**Description**: All errors return structured information: error category, error code, human-readable message, structured context (field name, constraint violated, input value). Structured errors enable meaningful client feedback, debugging, and automated error handling.

**Used In**:

- [Error Taxonomy](08-resilience/error-taxonomy.md) Section 3
- [Validation Hub](07-validation/README.md) Section 7
- [Resilience Hub](08-resilience/README.md) Section 11

### Exponential Backoff Retry Pattern

**Description**: Transient errors (network, temporary database) retry with exponential backoff: 1s, 2s, 4s, 8s, 16s, max 60s. Jitter prevents thundering herd. Maximum 5 retry attempts before giving up. Exponential backoff appropriate for transient errors only; persistent errors fail immediately.

**Used In**:

- [Error Taxonomy](08-resilience/error-taxonomy.md) Category 1, Category 4
- [Failure Modes](08-resilience/failure-modes.md) Section 4
- [Resilience Hub](08-resilience/README.md) Section 3

### Error Category Classification Pattern

**Description**: All errors classified into 6 categories with specific handling strategies. Classification enables consistent error handling across codebase, simplifies reasoning about error recovery, and ensures appropriate logging/retry/degradation for each category.

**Used In**:

- [Error Taxonomy](08-resilience/error-taxonomy.md) Section 1
- [Resilience Hub](08-resilience/README.md) Section 1
- [Logging Standards](08-resilience/logging-standards.md) Section 5

### Fail-Fast Validation Pattern

**Description**: Validation errors detected and reported at earliest possible layer. Invalid input rejected before expensive operations (database writes, rule evaluation). Fail-fast prevents invalid data from propagating through system layers, reducing debugging complexity.

**Used In**:

- [Validation Hub](07-validation/README.md) Section 6
- [Error Taxonomy](08-resilience/error-taxonomy.md) Category 6
- [Resilience Hub](08-resilience/README.md) Section 9

## Related Indexes

- **[Observability Index](observability-index.md)**: Error logging is a core observability concern. See observability index for structured error logging, severity levels, and error rate monitoring.
- **[Validation Index](validation-index.md)**: Validation errors (Category 6) are a key error category. See validation index for validation error structure, fail-fast patterns, and validation layer responsibilities.
- **[Performance Index](performance-index.md)**: Error handling has performance overhead. See performance index for error handling cost considerations and optimization strategies.
- **[Security Index](security-index.md)**: Security errors (authentication failures, authorization denials) require special handling. See security index for security-specific error patterns.

## Maintenance Notes

**Last Updated**: 2025-11-07
**Last Review**: 2025-11-07
**Next Review**: 2026-02-07 (quarterly)
**Maintainer**: Resilience Team

**Known Gaps**:

- Circuit breaker pattern (future consideration for database error handling)
- Bulkhead pattern (future consideration for resource isolation)
- Chaos engineering testing (failure injection not yet implemented)
- Error rate SLOs (service level objectives not yet defined)

**Planned Additions**:

- Error recovery runbooks (operational procedures for each error category)
- Error testing framework (property-based testing for error paths)
- Error analytics (which error categories occur most frequently)
