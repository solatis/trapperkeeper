---
doc_type: spoke
status: active
date_created: 2025-11-07
primary_category: deployment
hub_document: /Users/lmergen/git/trapperkeeper/doc/09-operations/README.md
tags:
  - health-checks
  - kubernetes
  - prometheus
  - monitoring
  - observability
---

# Health Check Endpoints

## Context

TrapperKeeper services must support container orchestration and monitoring systems through standardized health check endpoints. This document specifies liveness and readiness probes for Kubernetes, and Prometheus metrics endpoints for observability.

**Hub Document**: This spoke is part of [Operations Overview](README.md). See the hub's Health Check Endpoints section for strategic context.

## Standard Kubernetes Endpoints

### Endpoint Overview

**Both Services** (`tk-sensor-api` and `tk-web-ui`) provide:

- `/healthz`: Liveness check (process running)
- `/readyz`: Readiness check (database connectivity)

**Sensor API Only** (`tk-sensor-api`):

- `/api/v1/stats/prometheus`: Prometheus metrics endpoint

**Design Principle**: Unauthenticated endpoints simplify orchestration configuration (acceptable for internal networks).

### Liveness Check: /healthz

**Purpose**: Indicates service binary is alive and not deadlocked.

**Specification**:

- **HTTP Method**: GET
- **Path**: `/healthz`
- **Response Status**: 200 OK (always, if process running)
- **Response Body**: `{"status": "ok"}`
- **Content-Type**: `application/json`
- **Timeout**: Must respond within 1 second

**Implementation** (both services):

```go
func healthzHandler(w http.ResponseWriter, r *http.Request) {
    w.Header().Set("Content-Type", "application/json")
    w.WriteHeader(http.StatusOK)
    json.NewEncoder(w).Encode(map[string]string{
        "status": "ok",
    })
}

// Route registration
http.HandleFunc("/healthz", healthzHandler)
```

**Kubernetes Liveness Probe Configuration**:

```yaml
livenessProbe:
  httpGet:
    path: /healthz
    port: 8080 # or 50051 for sensor-api
  initialDelaySeconds: 10
  periodSeconds: 10
  timeoutSeconds: 1
  failureThreshold: 3
```

**Failure Behavior**: If liveness check fails 3 consecutive times, Kubernetes restarts the pod.

### Readiness Check: /readyz

**Purpose**: Indicates service is ready to receive traffic (database connectivity confirmed).

**Specification**:

- **HTTP Method**: GET
- **Path**: `/readyz`
- **Response Status**: 200 OK (ready) or 503 Service Unavailable (not ready)
- **Response Body**: `{"status": "ready"}` or `{"status": "unavailable", "error": "database connection failed"}`
- **Content-Type**: `application/json`
- **Timeout**: Must respond within 1 second
- **Database Check**: Ping database with 1-second timeout

**Implementation** (both services):

```go
import (
    "context"
    "database/sql"
    "encoding/json"
    "net/http"
    "time"
)

type ReadyzResponse struct {
    Status string  `json:"status"`
    Error  *string `json:"error,omitempty"`
}

func readyzHandler(db *sql.DB) http.HandlerFunc {
    return func(w http.ResponseWriter, r *http.Request) {
        w.Header().Set("Content-Type", "application/json")

        // Ping database with 1-second timeout
        ctx, cancel := context.WithTimeout(r.Context(), time.Second)
        defer cancel()

        if err := db.PingContext(ctx); err != nil {
            w.WriteHeader(http.StatusServiceUnavailable)
            errMsg := "database connection failed"
            json.NewEncoder(w).Encode(ReadyzResponse{
                Status: "unavailable",
                Error:  &errMsg,
            })
            return
        }

        w.WriteHeader(http.StatusOK)
        json.NewEncoder(w).Encode(ReadyzResponse{
            Status: "ready",
        })
    }
}

// Route registration
http.HandleFunc("/readyz", readyzHandler(db))
```

**Kubernetes Readiness Probe Configuration**:

```yaml
readinessProbe:
  httpGet:
    path: /readyz
    port: 8080 # or 50051 for sensor-api
  initialDelaySeconds: 5
  periodSeconds: 5
  timeoutSeconds: 1
  successThreshold: 1
  failureThreshold: 3
```

**Failure Behavior**: If readiness check fails 3 consecutive times, Kubernetes removes pod from service endpoints (no traffic routed).

**Rationale**: Prevents routing traffic to pods with database connectivity issues, avoiding cascading failures.

## Prometheus Metrics Endpoint

### Endpoint Specification

**Service**: `tk-sensor-api` only (web UI does not provide metrics in MVP)

**Specification**:

- **HTTP Method**: GET
- **Path**: `/api/v1/stats/prometheus`
- **Response Status**: 200 OK
- **Response Body**: Prometheus text exposition format
- **Content-Type**: `text/plain; version=0.0.4`
- **Authentication**: None (MVP), metrics accessible without authentication

**Security Consideration**: Metrics endpoint exposes operational statistics (event counts, storage size). No authentication required in MVP for internal network deployments.

### Metrics Included (MVP)

**Event Processing Metrics**:

```
# HELP trapperkeeper_events_received_total Total number of events received by action
# TYPE trapperkeeper_events_received_total counter
trapperkeeper_events_received_total{action="observe"} 12345
trapperkeeper_events_received_total{action="drop"} 678
trapperkeeper_events_received_total{action="error"} 12

# HELP trapperkeeper_events_received_bytes Total bytes received by action
# TYPE trapperkeeper_events_received_bytes counter
trapperkeeper_events_received_bytes{action="observe"} 5678901
trapperkeeper_events_received_bytes{action="drop"} 234567
trapperkeeper_events_received_bytes{action="error"} 8901
```

**Rule Metrics**:

```
# HELP trapperkeeper_rule_count Current number of active rules
# TYPE trapperkeeper_rule_count gauge
trapperkeeper_rule_count 42
```

**Storage Metrics**:

```
# HELP trapperkeeper_storage_size_bytes Event storage size in bytes
# TYPE trapperkeeper_storage_size_bytes gauge
trapperkeeper_storage_size_bytes 123456789
```

### Implementation

```go
import (
    "github.com/prometheus/client_golang/prometheus"
    "github.com/prometheus/client_golang/prometheus/promhttp"
)

type Metrics struct {
    eventsReceivedTotal *prometheus.CounterVec
    eventsReceivedBytes *prometheus.CounterVec
    ruleCount           prometheus.Gauge
    storageSizeBytes    prometheus.Gauge
}

func NewMetrics(reg prometheus.Registerer) *Metrics {
    m := &Metrics{
        eventsReceivedTotal: prometheus.NewCounterVec(
            prometheus.CounterOpts{
                Name: "trapperkeeper_events_received_total",
                Help: "Total number of events received by action",
            },
            []string{"action"},
        ),
        eventsReceivedBytes: prometheus.NewCounterVec(
            prometheus.CounterOpts{
                Name: "trapperkeeper_events_received_bytes",
                Help: "Total bytes received by action",
            },
            []string{"action"},
        ),
        ruleCount: prometheus.NewGauge(
            prometheus.GaugeOpts{
                Name: "trapperkeeper_rule_count",
                Help: "Current number of active rules",
            },
        ),
        storageSizeBytes: prometheus.NewGauge(
            prometheus.GaugeOpts{
                Name: "trapperkeeper_storage_size_bytes",
                Help: "Event storage size in bytes",
            },
        ),
    }

    reg.MustRegister(m.eventsReceivedTotal)
    reg.MustRegister(m.eventsReceivedBytes)
    reg.MustRegister(m.ruleCount)
    reg.MustRegister(m.storageSizeBytes)

    return m
}

// Route registration (sensor-api only)
reg := prometheus.NewRegistry()
metrics := NewMetrics(reg)
http.Handle("/api/v1/stats/prometheus", promhttp.HandlerFor(reg, promhttp.HandlerOpts{}))
```

### Prometheus Scrape Configuration

**Prometheus Configuration** (`prometheus.yml`):

```yaml
scrape_configs:
  - job_name: "trapperkeeper-sensor-api"
    scrape_interval: 15s
    static_configs:
      - targets: ["sensor-api:50051"]
    metrics_path: "/api/v1/stats/prometheus"
```

**Kubernetes Service Discovery**:

```yaml
scrape_configs:
  - job_name: "trapperkeeper-sensor-api"
    kubernetes_sd_configs:
      - role: pod
        namespaces:
          names:
            - trapperkeeper
    relabel_configs:
      - source_labels: [__meta_kubernetes_pod_label_app]
        action: keep
        regex: sensor-api
      - source_labels: [__meta_kubernetes_pod_ip]
        action: replace
        target_label: __address__
        replacement: ${1}:50051
      - source_labels: [__meta_kubernetes_pod_name]
        action: replace
        target_label: instance
```

## Container Orchestration Integration

### Kubernetes Deployment Example

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: trapperkeeper-sensor-api
spec:
  replicas: 3
  selector:
    matchLabels:
      app: sensor-api
  template:
    metadata:
      labels:
        app: sensor-api
    spec:
      containers:
        - name: sensor-api
          image: trapperkeeper:latest
          ports:
            - containerPort: 50051
          env:
            - name: TK_HMAC_SECRET
              valueFrom:
                secretKeyRef:
                  name: trapperkeeper-secrets
                  key: hmac-secret
          livenessProbe:
            httpGet:
              path: /healthz
              port: 50051
            initialDelaySeconds: 10
            periodSeconds: 10
            timeoutSeconds: 1
            failureThreshold: 3
          readinessProbe:
            httpGet:
              path: /readyz
              port: 50051
            initialDelaySeconds: 5
            periodSeconds: 5
            timeoutSeconds: 1
            failureThreshold: 3
```

### Docker Compose Health Checks

```yaml
version: "3.8"
services:
  sensor-api:
    image: trapperkeeper:latest
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:50051/healthz"]
      interval: 10s
      timeout: 1s
      retries: 3
      start_period: 10s
```

## Response Timing Requirements

### 1-Second Timeout

**Critical Requirement**: All health check endpoints MUST respond within 1 second.

**Rationale**:

- Fast health checks prevent cascading failures
- Orchestrators expect sub-second health check responses
- 1-second database ping timeout prevents blocking on network issues

**Implementation**:

```go
// Readiness check with explicit timeout
ctx, cancel := context.WithTimeout(context.Background(), 1*time.Second)
defer cancel()

conn, err := pool.Acquire(ctx)
if err != nil {
    // not ready (timeout or connection error)
    return false
}
conn.Release()
// ready
return true
```

**Failure Modes**:

- Database unavailable: Return 503 within 1 second (timeout)
- Database slow: Return 503 within 1 second (timeout)
- Network partition: Return 503 within 1 second (timeout)

## Metrics Cardinality Considerations

### Label Cardinality

**Action Label** (`trapperkeeper_events_received_total{action}`):

- Cardinality: 3 (observe, drop, error)
- No high-cardinality labels (no tenant_id, rule_id, user_id in MVP)

**Rationale**: High-cardinality labels cause Prometheus performance issues. Limit label values to low-cardinality dimensions.

**Future Considerations**: If per-tenant metrics needed, use separate Prometheus instance or time-series database with higher cardinality support.

## No Client/Sensor Metrics (MVP Scope)

**Excluded from MVP**:

- Client-side SDK metrics (event submission rates, buffer sizes)
- Sensor-level metrics (per-sensor event counts, error rates)
- Per-rule evaluation metrics (rule execution times, match rates)

**Rationale**: MVP focuses on service-level observability. Client/sensor metrics add complexity without immediate operational value for small deployments.

**Future Considerations**: Add client/sensor metrics post-MVP if operational needs justify complexity.

## Related Documents

**Dependencies** (read these first):

- [Operations Overview](README.md): Strategic context for health check endpoints
- [Architecture: Service Architecture](../02-architecture/README.md): Two-service architecture requiring health checks

**Related Spokes** (siblings in this hub):

- [Web Framework](web-framework.md): net/http integration with health check routes
- [Database Backend](database-backend.md): Database connectivity for readiness checks

**Observability References**:

- [Error Handling: Error Handling Strategy](../07-error-handling/README.md): Monitoring strategy and metrics collection (consolidated reference)
