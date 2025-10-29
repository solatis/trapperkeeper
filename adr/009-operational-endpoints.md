# ADR-009: Operational Endpoints

Date: 2025-10-28

## Related Decisions

**Depends on:**
- **ADR-006: Service Architecture** - Adds operational endpoints to both tk-sensor-api and tk-web-ui services for container orchestration support

## Context

Both TrapperKeeper services need to support container orchestration and monitoring systems. Kubernetes and similar platforms require standardized health check endpoints to determine when containers are ready to receive traffic and when they should be restarted. Monitoring systems like Prometheus need a standard endpoint for collecting metrics.

Requirements:
- Health checks for container orchestration (liveness and readiness)
- Metrics endpoint for Prometheus integration
- Fast response times (sub-second for health checks)
- No authentication required for operational endpoints

## Decision

### Health Check Endpoints (Both Services)

Both `tk-sensor-api` and `tk-web-ui` provide standard Kubernetes health check endpoints:

- **`/healthz`** - Liveness check
  - Always returns 200 OK if process is running
  - Indicates service binary is alive and not deadlocked
  - Used by orchestrators to determine when to restart container

- **`/readyz`** - Readiness check
  - Returns 200 OK if database is pingable
  - Returns 503 Service Unavailable if database connection fails
  - Response body on failure: `{"status": "unavailable", "error": "database connection failed"}`
  - Must respond within 1 second timeout
  - Used by orchestrators to determine when to send traffic

Health endpoints require no authentication to allow orchestration platforms unrestricted access.

### Prometheus Metrics (API Server Only)

The `tk-sensor-api` service provides:

- **`/api/v1/stats/prometheus`** - Prometheus metrics endpoint
  - Returns metrics in Prometheus text exposition format
  - No authentication required in MVP

**Metrics included:**
- `trapperkeeper_events_received_total{action}` - Counter by action (observe/drop/error)
- `trapperkeeper_events_received_bytes{action}` - Bytes received by action
- `trapperkeeper_rule_count` - Current number of active rules
- `trapperkeeper_storage_size_bytes` - Event storage size in bytes

The web UI does not provide Prometheus metrics. Client/sensor metrics are out of scope for MVP.

## Consequences

**Benefits:**
- Standard Kubernetes semantics for health checks
- Container orchestrators can properly manage service lifecycle
- Prometheus integration provides basic observability
- Fast health checks prevent cascading failures
- Unauthenticated endpoints simplify orchestration configuration

**Tradeoffs:**
- Health endpoints exposed without authentication (acceptable for internal networks)
- Limited metrics in MVP (can expand later based on operational needs)
- Web UI lacks metrics endpoint (can add if monitoring needs arise)

This provides essential operational support while maintaining simplicity for MVP.
