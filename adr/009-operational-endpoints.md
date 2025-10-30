# ADR-009: Operational Endpoints

## Revision Log

| Date | Description |
|------|-------------|
| 2025-10-28 | Document created |

## Context

Both TrapperKeeper services need to support container orchestration and monitoring systems. Kubernetes and similar platforms require standardized health check endpoints to determine when containers are ready to receive traffic and when they should be restarted.

## Decision

We will implement standard Kubernetes health check endpoints (`/healthz` and `/readyz`) in both services and a Prometheus metrics endpoint (`/api/v1/stats/prometheus`) in the API server only.

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

**Operational Implications:**
- Health endpoints must respond within 1 second timeout
- Database connectivity determines readiness status
- No authentication required for operational endpoints

## Implementation

1. Implement `/healthz` endpoint in both `tk-sensor-api` and `tk-web-ui` that returns 200 OK if process is running
2. Implement `/readyz` endpoint in both services that pings database and returns 200 OK or 503 based on connectivity
3. Add timeout handling to ensure readiness checks respond within 1 second
4. Implement `/api/v1/stats/prometheus` endpoint in `tk-sensor-api` that exposes metrics in Prometheus text format
5. Configure container orchestration platforms to use these endpoints for lifecycle management

## Related Decisions

This ADR adds operational endpoints to both tk-sensor-api and tk-web-ui services for container orchestration support.

**Depends on:**
- **ADR-006: Service Architecture** - Defines the two-service architecture that these endpoints are added to

## Appendix A: Health Check Endpoint Specifications

### Liveness Check (`/healthz`)

Both `tk-sensor-api` and `tk-web-ui` provide this endpoint:

- Always returns 200 OK if process is running
- Indicates service binary is alive and not deadlocked
- Used by orchestrators to determine when to restart container

### Readiness Check (`/readyz`)

Both `tk-sensor-api` and `tk-web-ui` provide this endpoint:

- Returns 200 OK if database is pingable
- Returns 503 Service Unavailable if database connection fails
- Response body on failure: `{"status": "unavailable", "error": "database connection failed"}`
- Must respond within 1 second timeout
- Used by orchestrators to determine when to send traffic

## Appendix B: Prometheus Metrics Specification

The `tk-sensor-api` service provides:

**Endpoint:** `/api/v1/stats/prometheus`
- Returns metrics in Prometheus text exposition format
- No authentication required in MVP

**Metrics included:**
- `trapperkeeper_events_received_total{action}` - Counter by action (observe/drop/error)
- `trapperkeeper_events_received_bytes{action}` - Bytes received by action
- `trapperkeeper_rule_count` - Current number of active rules
- `trapperkeeper_storage_size_bytes` - Event storage size in bytes

**Note:** The web UI does not provide Prometheus metrics. Client/sensor metrics are out of scope for MVP.
