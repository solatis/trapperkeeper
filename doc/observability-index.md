---
doc_type: index
status: active
date_updated: 2025-11-07
primary_category: operations
cross_cutting:
  - observability
maintainer: Operations Team
last_review: 2025-11-07
next_review: 2026-02-07
---

# Observability Index

## Purpose

This index provides navigation to all documentation addressing **observability** across the Trapperkeeper system. Use this as a discovery mechanism for logging, tracing, metrics, and monitoring patterns regardless of their primary domain. Observability is critical for production operations, debugging, performance analysis, and SOC2 compliance.

## Quick Reference

| Category          | Description                                                  | Key Documents                                                                                                        |
| ----------------- | ------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------- |
| Logging Standards | slog structured logging, log levels, contextual fields       | [Error Taxonomy](08-resilience/error-taxonomy.md) Section 4, [Logging Standards](08-resilience/logging-standards.md) |
| Tracing           | Distributed tracing with span context, request correlation   | [Logging Standards](08-resilience/logging-standards.md) Section 2                                                    |
| Metrics           | Performance metrics, error rates, rule evaluation statistics | [Monitoring Strategy](08-resilience/monitoring-strategy.md)                                                          |
| Health Endpoints  | Readiness/liveness probes for container orchestration        | [Health Endpoints](09-operations/health-endpoints.md)                                                                |
| Alert Thresholds  | Alert rules, escalation policies, on-call runbooks           | [Monitoring Strategy](08-resilience/monitoring-strategy.md) Section 3                                                |

## Core Concepts

### slog Structured Logging Integration

TrapperKeeper uses Go's `slog` package for structured logging and distributed tracing. All log events use structured format with consistent field names, severity levels (ERROR, WARN, INFO, DEBUG), and span context for request correlation. Structured logging enables log aggregation, filtering, and analysis in production environments.

**Relevant Documentation:**

- **[Logging Standards](08-resilience/logging-standards.md)** - Complete slog integration patterns -> See Section 1 for structured logging format
- **[Error Taxonomy](08-resilience/error-taxonomy.md)** - Error category logging standards -> See Section 4 for severity level mapping
- **[Monitoring Strategy](08-resilience/monitoring-strategy.md)** - How logs integrate with monitoring -> See Section 2 for log aggregation patterns

### Structured Logging Format

All log events follow consistent structured format: timestamp (ISO 8601), severity level (ERROR/WARN/INFO/DEBUG), span context (request_id, trace_id), component (rule_engine, api_server, database), message (human-readable), and structured fields (key-value pairs for filtering). Structured format enables automated log analysis and alerting.

**Relevant Documentation:**

- **[Logging Standards](08-resilience/logging-standards.md)** - Complete structured format specification → See Section 3 for field definitions
- **[Error Taxonomy](08-resilience/error-taxonomy.md)** - Error-specific structured fields → See Section 4 for error context fields
- **[Monitoring Strategy](08-resilience/monitoring-strategy.md)** - How structured logs enable alerting → See Section 4 for alert rule patterns

### Distributed Tracing

TrapperKeeper uses distributed tracing to correlate logs across service boundaries. Each request receives a unique `request_id` (UUIDv7) that propagates through all log events. Span context includes parent spans for nested operations. Distributed tracing enables end-to-end request tracking from sensor API calls through rule evaluation to event storage.

**Relevant Documentation:**

- **[Logging Standards](08-resilience/logging-standards.md)** - Distributed tracing implementation → See Section 2 for span context propagation
- **[Identifiers](03-data/identifiers-uuidv7.md)** - UUIDv7 usage for request IDs → See Section 2 for request correlation
- **[API Service](02-architecture/api-service.md)** - How request IDs propagate through gRPC → See Section 4 for metadata headers

### Log Severity Levels

TrapperKeeper uses four log severity levels: ERROR (system failures requiring immediate attention), WARN (degraded functionality, potential issues), INFO (significant events, state changes), DEBUG (detailed diagnostic information). Production deployments typically use INFO level; DEBUG is development-only due to performance overhead.

**Relevant Documentation:**

- **[Logging Standards](08-resilience/logging-standards.md)** - Complete severity level definitions → See Section 4 for level selection guidance
- **[Error Taxonomy](08-resilience/error-taxonomy.md)** - Error category to severity level mapping → See Section 4 for severity guidelines
- **[Monitoring Strategy](08-resilience/monitoring-strategy.md)** - Alert thresholds by severity level → See Section 3 for severity-based alerting

### Health Endpoints

TrapperKeeper exposes health check endpoints for container orchestration (Kubernetes, Docker Swarm). Readiness endpoint (`/health/ready`) indicates service is ready to accept traffic. Liveness endpoint (`/health/live`) indicates service is running but may not be ready. Health endpoints return HTTP 200 (healthy) or 503 (unhealthy) with JSON payload indicating component status (database, gRPC server, web server).

**Relevant Documentation:**

- **[Health Endpoints](09-operations/health-endpoints.md)** - Complete health check specification → See Section 2 for endpoint definitions
- **[Service Architecture](02-architecture/service-architecture.md)** - How health endpoints integrate with services → See Section 5 for health check integration
- **[Monitoring Strategy](08-resilience/monitoring-strategy.md)** - Health endpoint monitoring → See Section 5 for orchestration patterns

### Performance Metrics

TrapperKeeper collects performance metrics for rule evaluation: evaluation latency (p50, p90, p99), rule match rates, sampling rates, evaluation errors, and cost model predictions vs actuals. Metrics enable performance monitoring, capacity planning, and optimization identification.

**Relevant Documentation:**

- **[Monitoring Strategy](08-resilience/monitoring-strategy.md)** - Complete metrics specification → See Section 6 for performance metrics
- **[Performance Hub](05-performance/README.md)** - How metrics relate to cost model → See Section 8 for metrics-driven optimization
- **[Sampling Strategies](05-performance/sampling.md)** - Sampling rate metrics → See Section 4 for sampling observability

### Error Rate Monitoring

TrapperKeeper monitors error rates across all error categories: network errors, type coercion errors, missing field errors, database errors, protocol errors, validation errors. Error rate thresholds trigger alerts when error rates exceed acceptable levels. Separate thresholds for transient errors (network) vs persistent errors (configuration).

**Relevant Documentation:**

- **[Monitoring Strategy](08-resilience/monitoring-strategy.md)** - Error rate alerting → See Section 3 for alert threshold definitions
- **[Error Taxonomy](08-resilience/error-taxonomy.md)** - Error categories monitored → See Section 1 for complete error taxonomy
- **[Logging Standards](08-resilience/logging-standards.md)** - How errors are logged for monitoring → See Section 5 for error event structure

### Alert Thresholds and Escalation

Alert thresholds defined for critical metrics: error rate >5% (critical), evaluation latency p99 >10ms (warning), database connection failures >3 in 5 minutes (critical), health check failures >2 consecutive (critical). Escalation policies route alerts to appropriate teams (operations for infrastructure, development for application errors).

**Relevant Documentation:**

- **[Monitoring Strategy](08-resilience/monitoring-strategy.md)** - Complete alert threshold specifications → See Section 3 for all thresholds
- **[Error Taxonomy](08-resilience/error-taxonomy.md)** - Which errors trigger alerts → See Section 5 for alertable error categories
- **[Health Endpoints](09-operations/health-endpoints.md)** - Health check alert integration → See Section 3 for alert patterns

## Domain Coverage Matrix

| Domain         | Coverage | Key Document                                                                       |
| -------------- | -------- | ---------------------------------------------------------------------------------- |
| Architecture   | ✓        | [Service Architecture](02-architecture/service-architecture.md)                    |
| API Design     | ✓        | [API Service](02-architecture/api-service.md) (request tracing)                    |
| Database       | ✓        | [Database Backend](09-operations/database-backend.md) (query logging)              |
| Security       | ✓        | [Security Hub](06-security/README.md) (security event logging)                     |
| Performance    | ✓        | [Performance Hub](05-performance/README.md) (performance metrics)                  |
| Validation     | ✓        | [Validation Hub](07-validation/README.md) (validation error logging)               |
| Configuration  | ✓        | [Configuration Management](09-operations/configuration.md) (config change logging) |
| Testing        | ✓        | [Testing Philosophy](01-principles/testing-philosophy.md) (test observability)     |
| Deployment     | ✓        | [Health Endpoints](09-operations/health-endpoints.md)                              |
| Error Handling | ✓        | [Error Taxonomy](08-resilience/error-taxonomy.md)                                  |

## Patterns and Best Practices

### Structured Logging Pattern

**Description**: All log events use structured format with consistent field names and types. Structured fields enable automated log analysis, filtering, and alerting. Human-readable message combined with machine-parseable fields provides both operational visibility and automation capability.

**Used In**:

- [Logging Standards](08-resilience/logging-standards.md) Section 3
- [Error Taxonomy](08-resilience/error-taxonomy.md) Section 4
- [Monitoring Strategy](08-resilience/monitoring-strategy.md) Section 2

### Distributed Tracing Pattern

**Description**: Request correlation using UUIDv7 request IDs propagated through all log events. Span context enables end-to-end request tracking across service boundaries. Parent-child span relationships capture nested operation hierarchies.

**Used In**:

- [Logging Standards](08-resilience/logging-standards.md) Section 2
- [Identifiers](03-data/identifiers-uuidv7.md) Section 2
- [API Service](02-architecture/api-service.md) Section 4

### Severity-Based Alerting Pattern

**Description**: Log severity levels map to alert priorities. ERROR logs trigger immediate alerts. WARN logs aggregate to periodic summaries. INFO logs provide operational visibility without alerting. DEBUG/TRACE logs are development-only.

**Used In**:

- [Monitoring Strategy](08-resilience/monitoring-strategy.md) Section 3
- [Logging Standards](08-resilience/logging-standards.md) Section 4
- [Error Taxonomy](08-resilience/error-taxonomy.md) Section 4

### Health Check Pattern

**Description**: Readiness and liveness probes enable container orchestration. Readiness indicates service can accept traffic (dependencies healthy). Liveness indicates service is running (process alive). Separate endpoints enable different restart policies.

**Used In**:

- [Health Endpoints](09-operations/health-endpoints.md)
- [Service Architecture](02-architecture/service-architecture.md) Section 5
- [Monitoring Strategy](08-resilience/monitoring-strategy.md) Section 5

### Metrics-Driven Optimization Pattern

**Description**: Performance metrics (evaluation latency, rule match rates) inform optimization decisions. Cost model predictions compared against actual metrics to validate performance model. Metrics identify high-cost rules for optimization.

**Used In**:

- [Monitoring Strategy](08-resilience/monitoring-strategy.md) Section 6
- [Performance Hub](05-performance/README.md) Section 8
- [Cost Model](05-performance/cost-model.md) Section 6

## Related Indexes

- **[Error Handling Index](error-handling-index.md)**: Error logging is a core observability concern. See error handling index for error taxonomy, severity levels, and structured error logging patterns.
- **[Performance Index](performance-index.md)**: Performance metrics are key observability signals. See performance index for evaluation latency metrics, cost model tracking, and optimization identification.
- **[Security Index](security-index.md)**: Security event logging (authentication failures, authorization denials) is critical for security monitoring. See security index for security-specific logging patterns.

## Maintenance Notes

**Last Updated**: 2025-11-07
**Last Review**: 2025-11-07
**Next Review**: 2026-02-07 (quarterly)
**Maintainer**: Operations Team

**Known Gaps**:

- OpenTelemetry integration (future consideration for standardized tracing)
- Metrics export to Prometheus/Grafana (export format not yet specified)
- Log retention policies (retention duration not yet defined)
- On-call runbooks (alert response procedures not yet documented)

**Planned Additions**:

- Observability testing framework (synthetic monitoring, chaos engineering)
- Log analysis patterns (common queries, dashboard templates)
- Performance profiling integration (flamegraphs, CPU profiles)
