---
doc_type: spoke
status: active
primary_category: observability
hub_document: doc/08-resilience/README.md
tags:
  - monitoring
  - alerting
  - metrics
  - observability
  - prometheus
---

# Monitoring Strategy

## Context

Proactive monitoring and alerting are essential for maintaining system reliability, detecting issues before users are affected, and enabling rapid incident response. Without comprehensive monitoring strategy, operators lack visibility into system health, error rates, and performance degradation.

This document establishes monitoring strategy for TrapperKeeper including metrics specifications, alert thresholds, alert destinations, and incident response procedures. It integrates with error taxonomy to provide category-specific alerting and with logging standards to enable log-based metrics.

**Hub Document**: This document is part of the Resilience Architecture hub. See [README.md](README.md) for strategic overview of error handling principles, error taxonomy, and decision trees integrating monitoring thresholds.

## Metrics Framework: Prometheus

**ABSOLUTE REQUIREMENT**: Expose all metrics via Prometheus exposition format.

**Rationale**: Prometheus is industry-standard, supports multi-dimensional metrics with labels, integrates with Grafana for visualization, and provides flexible alerting via Alertmanager.

**Exposition Endpoint**: `/metrics` (exposed per Operational Endpoints specification)

**Dependencies**:

```bash
go get github.com/prometheus/client_golang/prometheus
go get github.com/prometheus/client_golang/prometheus/promhttp
```

**Initialization**:

```go
import (
    "github.com/prometheus/client_golang/prometheus"
    "github.com/prometheus/client_golang/prometheus/promhttp"
)

func initMetrics(reg prometheus.Registerer) *Metrics {
    errorsTotal := prometheus.NewCounterVec(
        prometheus.CounterOpts{
            Name: "errors_total",
            Help: "Total error count by category and severity",
        },
        []string{"category", "severity"},
    )

    requestDuration := prometheus.NewHistogramVec(
        prometheus.HistogramOpts{
            Name: "request_duration_seconds",
            Help: "Request duration in seconds",
        },
        []string{"endpoint", "method"},
    )

    reg.MustRegister(errorsTotal)
    reg.MustRegister(requestDuration)

    return &Metrics{
        errorsTotal:     errorsTotal,
        requestDuration: requestDuration,
    }
}
```

## Metrics Specifications

Metrics follow Prometheus naming conventions with consistent labeling strategy.

### Error Rate Metrics

#### `errors_total` (Counter)

**Purpose**: Track total error count by category and severity.

**Type**: Counter (monotonically increasing)

**Labels**:

- `category`: Error category (`network`, `database`, `protocol`, `validation`, `type_coercion`, `missing_field`)
- `severity`: Log level (`error`, `warn`, `info`)

**Usage**:

```go
// Increment on error occurrence
metrics.errorsTotal.
    WithLabelValues("database", "error").
    Inc()

// Increment with count
metrics.errorsTotal.
    WithLabelValues("network", "error").
    Add(float64(consecutiveFailures))
```

**PromQL Queries**:

```promql
# Total errors per minute
rate(errors_total[1m])

# Error rate by category
sum(rate(errors_total[5m])) by (category)

# Critical errors (ERROR severity only)
rate(errors_total{severity="error"}[5m])
```

**Alert Integration**: Used for all category-specific alert thresholds (database errors, network failures, validation errors).

#### `http_errors_total` (Counter)

**Purpose**: Track HTTP errors by endpoint and status code.

**Type**: Counter

**Labels**:

- `endpoint`: API endpoint path (`/api/rules`, `/api/events`, `/health`)
- `status_code`: HTTP status code (`400`, `422`, `500`, `503`)
- `method`: HTTP method (`GET`, `POST`, `PUT`, `DELETE`)

**Usage**:

```go
metrics.httpErrorsTotal.
    WithLabelValues("/api/rules", "422", "POST").
    Inc()
```

**PromQL Queries**:

```promql
# 5xx error rate
sum(rate(http_errors_total{status_code=~"5.."}[5m])) by (endpoint)

# 4xx vs 5xx ratio
sum(rate(http_errors_total{status_code=~"4.."}[5m])) /
sum(rate(http_errors_total{status_code=~"5.."}[5m]))

# Top endpoints by error rate
topk(5, sum(rate(http_errors_total[5m])) by (endpoint))
```

**Alert Integration**: Triggers HIGH alert when 5xx rate exceeds 10% over 5 minutes.

#### `grpc_errors_total` (Counter)

**Purpose**: Track gRPC errors by method and status code.

**Type**: Counter

**Labels**:

- `method`: gRPC method name (`/tk.SensorAPI/SyncRules`, `/tk.SensorAPI/PostEvent`)
- `code`: gRPC status code (`INVALID_ARGUMENT`, `UNAVAILABLE`, `INTERNAL`)

**Usage**:

```go
metrics.grpcErrorsTotal.
    WithLabelValues("SyncRules", "UNAVAILABLE").
    Inc()
```

**PromQL Queries**:

```promql
# gRPC error rate by method
rate(grpc_errors_total[5m])

# INTERNAL errors (server-side issues)
rate(grpc_errors_total{code="INTERNAL"}[5m])
```

**Alert Integration**: Used for network degradation and service availability alerts.

#### `database_errors_total` (Counter)

**Purpose**: Track database errors by operation type.

**Type**: Counter

**Labels**:

- `operation`: Operation type (`query`, `insert`, `update`, `delete`, `transaction`, `migration`)
- `error_type`: Error classification (`connection_failure`, `constraint_violation`, `timeout`, `disk_full`)

**Usage**:

```go
metrics.databaseErrorsTotal.
    WithLabelValues("query", "connection_failure").
    Inc()
```

**PromQL Queries**:

```promql
# Any database error (should be 0)
rate(database_errors_total[1m])

# Database errors by operation type
sum(rate(database_errors_total[5m])) by (operation)
```

**Alert Integration**: Triggers CRITICAL alert on ANY database error (fail-fast pattern requires immediate intervention).

#### `validation_errors_total` (Counter)

**Purpose**: Track validation errors by field and error type.

**Type**: Counter

**Labels**:

- `field`: Field that failed validation (`conditions[0].operator`, `rule_name`, `user_email`)
- `error_type`: Validation error type (`invalid_value`, `missing_required`, `format_error`, `out_of_range`)

**Usage**:

```go
metrics.validationErrorsTotal.
    WithLabelValues("conditions[0].operator", "invalid_value").
    Inc()
```

**PromQL Queries**:

```promql
# Validation error rate by field
rate(validation_errors_total[1h])

# Top fields by error rate (indicates UX problems)
topk(5, sum(rate(validation_errors_total[1h])) by (field))
```

**Alert Integration**: Triggers MEDIUM alert when specific field error rate exceeds 20% over 1 hour (indicates UX problem or client bug).

### Network Failure Metrics

#### `network_consecutive_failures` (Gauge)

**Purpose**: Track consecutive failure count for each endpoint.

**Type**: Gauge (can increase or decrease)

**Labels**:

- `endpoint`: Target endpoint URL (`https://api.trapperkeeper.io/sync`)

**Usage**:

```go
// Increment on failure
metrics.networkConsecutiveFailures.
    WithLabelValues(endpointURL).
    Inc()

// Reset on success
metrics.networkConsecutiveFailures.
    WithLabelValues(endpointURL).
    Set(0.0)
```

**PromQL Queries**:

```promql
# Endpoints with sustained failures
network_consecutive_failures > 10

# Max consecutive failures across all endpoints
max(network_consecutive_failures)
```

**Alert Integration**: Triggers HIGH alert when consecutive failures exceed 10 for any endpoint.

#### `network_failure_streak_seconds` (Gauge)

**Purpose**: Track duration of current failure streak for each endpoint.

**Type**: Gauge

**Labels**:

- `endpoint`: Target endpoint URL

**Usage**:

```go
// Update on failure
streakDuration := time.Since(failureStartTime).Seconds()
metrics.networkFailureStreakSeconds.
    WithLabelValues(endpointURL).
    Set(streakDuration)

// Reset on success
metrics.networkFailureStreakSeconds.
    WithLabelValues(endpointURL).
    Set(0.0)
```

**PromQL Queries**:

```promql
# Endpoints with failures lasting >5 minutes
network_failure_streak_seconds > 300

# Total time in failure state across all endpoints
sum(network_failure_streak_seconds)
```

**Alert Integration**: Used for escalation (HIGH → CRITICAL) when failure duration exceeds thresholds.

### Performance Metrics

#### `request_duration_seconds` (Histogram)

**Purpose**: Track request duration for latency monitoring.

**Type**: Histogram (with buckets)

**Labels**:

- `endpoint`: API endpoint path
- `method`: HTTP method or gRPC method

**Buckets**: `[0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 10.0]` (seconds)

**Usage**:

```go
timer := prometheus.NewTimer(
    metrics.requestDuration.
        WithLabelValues("/api/rules", "POST"),
)

// Process request
result := processRequest(request)

timer.ObserveDuration()  // Automatically records duration
```

**PromQL Queries**:

```promql
# p50, p95, p99 latency by endpoint
histogram_quantile(0.50, rate(request_duration_seconds_bucket[5m]))
histogram_quantile(0.95, rate(request_duration_seconds_bucket[5m]))
histogram_quantile(0.99, rate(request_duration_seconds_bucket[5m]))

# Average request duration
rate(request_duration_seconds_sum[5m]) /
rate(request_duration_seconds_count[5m])
```

**Alert Integration**: Triggers MEDIUM alert when p95 latency exceeds SLO thresholds.

#### `rule_evaluation_duration_seconds` (Histogram)

**Purpose**: Track rule evaluation duration for performance optimization.

**Type**: Histogram

**Labels**:

- `rule_id`: Rule UUID (high cardinality, use sampling)
- `condition_count`: Number of conditions in rule

**Buckets**: `[0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1]` (seconds)

**Usage**:

```go
timer := prometheus.NewTimer(
    metrics.ruleEvaluationDuration.
        WithLabelValues(rule.ID.String(), strconv.Itoa(conditionCount)),
)

result := evaluateRule(rule, event)

timer.ObserveDuration()
```

**PromQL Queries**:

```promql
# Slowest rules (p99 latency)
topk(10, histogram_quantile(0.99, rate(rule_evaluation_duration_seconds_bucket[5m])))

# Average evaluation time by condition count
avg(rate(rule_evaluation_duration_seconds_sum[5m])) by (condition_count)
```

**Alert Integration**: Used for performance analysis, not alerting (sampling prevents accurate alerting).

### Resource Utilization Metrics

#### `database_connection_pool_size` (Gauge)

**Purpose**: Track database connection pool utilization.

**Type**: Gauge

**Labels**:

- `state`: Connection state (`active`, `idle`, `waiting`)

**Usage**:

```go
metrics.databaseConnectionPoolSize.
    WithLabelValues("active").
    Set(float64(pool.Stats().InUse))

metrics.databaseConnectionPoolSize.
    WithLabelValues("idle").
    Set(float64(pool.Stats().Idle))
```

**PromQL Queries**:

```promql
# Connection pool exhaustion risk
database_connection_pool_size{state="active"} /
(database_connection_pool_size{state="active"} + database_connection_pool_size{state="idle"}) > 0.9
```

**Alert Integration**: Triggers MEDIUM alert when pool utilization exceeds 90% (risk of connection exhaustion).

#### `rule_cache_entries` (Gauge)

**Purpose**: Track sensor rule cache size.

**Type**: Gauge

**Labels**:

- `sensor_id`: Sensor identifier

**Usage**:

```go
metrics.ruleCacheEntries.
    WithLabelValues(sensor.ID.String()).
    Set(float64(cache.Len()))
```

**PromQL Queries**:

```promql
# Total cached rules across all sensors
sum(rule_cache_entries)

# Sensors with largest cache
topk(10, rule_cache_entries)
```

**Alert Integration**: Not alerting directly, used for capacity planning.

### Business Metrics

#### `rules_created_total` (Counter)

**Purpose**: Track rule creation events.

**Type**: Counter

**Labels**:

- `user_id`: User who created rule
- `state`: Initial rule state (`draft`, `active`)

**Usage**:

```go
metrics.rulesCreatedTotal.
    WithLabelValues(user.ID.String(), "draft").
    Inc()
```

**PromQL Queries**:

```promql
# Rules created per hour
rate(rules_created_total[1h]) * 3600

# Most active users by rule creation
topk(10, sum(rate(rules_created_total[1h])) by (user_id))
```

**Alert Integration**: Not alerting, used for usage analytics.

#### `events_processed_total` (Counter)

**Purpose**: Track event processing volume.

**Type**: Counter

**Labels**:

- `sensor_id`: Sensor that processed event
- `matched`: Whether event matched any rules (`true`, `false`)

**Usage**:

```go
metrics.eventsProcessedTotal.
    WithLabelValues(sensor.ID.String(), "true").
    Inc()
```

**PromQL Queries**:

```promql
# Events per second
rate(events_processed_total[1m])

# Match rate (percentage of events matching rules)
sum(rate(events_processed_total{matched="true"}[5m])) /
sum(rate(events_processed_total[5m]))
```

**Alert Integration**: Used for capacity planning and anomaly detection.

## Alert Thresholds

Alert thresholds follow error taxonomy categories with severity-based escalation.

### CRITICAL Severity Alerts

**Definition**: Immediate operator intervention required. Page on-call engineer via PagerDuty/SMS.

#### Database Errors (ANY Error)

**Metric**: `database_errors_total`

**Threshold**: `rate(database_errors_total[1m]) > 0`

**Rationale**: Database errors indicate system-level failure (connection loss, disk full, constraint violation). Fail-fast pattern requires immediate intervention.

**Alert Rule** (Prometheus Alertmanager):

```yaml
groups:
  - name: database
    rules:
      - alert: DatabaseErrorsDetected
        expr: rate(database_errors_total[1m]) > 0
        for: 0s # Alert immediately
        labels:
          severity: critical
        annotations:
          summary: "Database errors detected"
          description: "Database operation failed: {{ $labels.operation }} ({{ $labels.error_type }})"
          runbook: "https://docs.trapperkeeper.io/runbooks/database-errors"
```

**Incident Response**:

1. Check database health (`systemctl status postgresql`)
2. Verify connectivity (`psql -h localhost -U trapperkeeper`)
3. Check disk space (`df -h`)
4. Review connection pool metrics
5. Check recent migrations (if migration error)

**Escalation**: If not resolved within 15 minutes, escalate to engineering manager.

### HIGH Severity Alerts

**Definition**: Degraded functionality requiring attention within 30 minutes. Slack alert + email.

#### Network Sustained Failures (>10 Consecutive)

**Metric**: `network_consecutive_failures`

**Threshold**: `network_consecutive_failures > 10`

**Rationale**: >10 consecutive failures indicate sustained network partition or endpoint unavailability requiring investigation.

**Alert Rule**:

```yaml
- alert: NetworkSustainedFailures
  expr: network_consecutive_failures > 10
  for: 1m # Sustained for 1 minute
  labels:
    severity: high
  annotations:
    summary: "Sustained network failures to {{ $labels.endpoint }}"
    description: "{{ $value }} consecutive failures to {{ $labels.endpoint }}"
    runbook: "https://docs.trapperkeeper.io/runbooks/network-failures"
```

**Incident Response**:

1. Check endpoint availability (`curl -I https://endpoint`)
2. Check DNS resolution (`dig endpoint`)
3. Review network logs for patterns
4. Check TLS certificate validity
5. Verify API key validity (if authentication error)

**Escalation**: If not resolved within 30 minutes, escalate to HIGH priority page.

#### Protocol 5xx Rate (>10% over 5 minutes)

**Metric**: `http_errors_total{status_code=~"5.."}`

**Threshold**: `rate(http_errors_total{status_code=~"5.."}[5m]) / rate(http_requests_total[5m]) > 0.1`

**Rationale**: >10% 5xx rate indicates server-side issues (resource exhaustion, service crashes, database errors).

**Alert Rule**:

```yaml
- alert: HighServerErrorRate
  expr: |
    sum(rate(http_errors_total{status_code=~"5.."}[5m])) by (endpoint) /
    sum(rate(http_requests_total[5m])) by (endpoint) > 0.1
  for: 5m
  labels:
    severity: high
  annotations:
    summary: "High 5xx error rate on {{ $labels.endpoint }}"
    description: "{{ $value | humanizePercentage }} of requests failing with 5xx errors"
    runbook: "https://docs.trapperkeeper.io/runbooks/5xx-errors"
```

**Incident Response**:

1. Check service health (`systemctl status tk-web-ui tk-sensor-api`)
2. Review ERROR logs for patterns
3. Check resource utilization (CPU, memory, disk)
4. Review recent deployments (rollback if necessary)
5. Check database connection pool

**Escalation**: If error rate exceeds 20%, escalate to CRITICAL.

### MEDIUM Severity Alerts

**Definition**: Potential issues requiring investigation within 2 hours. Email digest.

#### Validation Errors (>20% for Specific Field over 1 Hour)

**Metric**: `validation_errors_total`

**Threshold**: `rate(validation_errors_total[1h]) / rate(http_requests_total[1h]) > 0.2` (per field)

**Rationale**: >20% validation error rate for specific field indicates UX problem (unclear error messages, confusing UI) or client bug.

**Alert Rule**:

```yaml
- alert: HighFieldValidationErrorRate
  expr: |
    sum(rate(validation_errors_total[1h])) by (field) /
    sum(rate(http_requests_total[1h])) > 0.2
  for: 1h
  labels:
    severity: medium
  annotations:
    summary: "High validation error rate for {{ $labels.field }}"
    description: "{{ $value | humanizePercentage }} validation errors for field {{ $labels.field }}"
    runbook: "https://docs.trapperkeeper.io/runbooks/validation-errors"
```

**Incident Response**:

1. Review error messages for clarity
2. Check UI for confusing elements
3. Review recent UI changes
4. Contact users with high error rates
5. Update documentation if needed

**Escalation**: If error rate exceeds 50%, escalate to HIGH (indicates major UX issue).

#### Missing Field Rate (>50% for Rule over 1 Hour)

**Metric**: Log-based metric (extracted from `rule_evaluation` logs)

**Threshold**: Missing field rate >50% for specific rule over 1 hour

**Rationale**: >50% missing field rate suggests schema drift (event schemas changed in ways affecting rule effectiveness).

**Alert Rule**:

```yaml
- alert: HighMissingFieldRate
  expr: |
    sum(rate(rule_evaluation_missing_fields_total[1h])) by (rule_id) /
    sum(rate(rule_evaluation_attempts_total[1h])) by (rule_id) > 0.5
  for: 1h
  labels:
    severity: medium
  annotations:
    summary: "High missing field rate for rule {{ $labels.rule_id }}"
    description: "{{ $value | humanizePercentage }} missing field rate"
    runbook: "https://docs.trapperkeeper.io/runbooks/missing-fields"
```

**Incident Response**:

1. Review rule definition (affected field paths)
2. Check recent event schema changes
3. Contact rule owner for investigation
4. Update rule to handle schema evolution
5. Consider adding default values

**Escalation**: Not typically escalated (owner notification sufficient).

#### Connection Pool Utilization (>90%)

**Metric**: `database_connection_pool_size`

**Threshold**: `database_connection_pool_size{state="active"} / (database_connection_pool_size{state="active"} + database_connection_pool_size{state="idle"}) > 0.9`

**Rationale**: >90% pool utilization risks connection exhaustion leading to request failures.

**Alert Rule**:

```yaml
- alert: ConnectionPoolHighUtilization
  expr: |
    database_connection_pool_size{state="active"} /
    (database_connection_pool_size{state="active"} + database_connection_pool_size{state="idle"}) > 0.9
  for: 5m
  labels:
    severity: medium
  annotations:
    summary: "Database connection pool high utilization"
    description: "Connection pool {{ $value | humanizePercentage }} utilized"
    runbook: "https://docs.trapperkeeper.io/runbooks/connection-pool"
```

**Incident Response**:

1. Check for slow queries (`pg_stat_activity`)
2. Check for connection leaks (not released after use)
3. Review connection pool configuration (max size)
4. Consider increasing pool size (if capacity allows)
5. Optimize slow queries

**Escalation**: If pool exhaustion causes request failures, escalate to HIGH.

## Alert Destinations

### Alert Routing by Severity

**CRITICAL** → PagerDuty → SMS to on-call engineer
**HIGH** → Slack `#alerts` channel + Email to team
**MEDIUM** → Email digest (hourly) + Grafana dashboard annotation

### Alert Aggregation

**Grouping**: Alerts grouped by `alertname` and `severity` to prevent notification storms.

**Inhibition**: Lower severity alerts inhibited when higher severity alerts active (e.g., HIGH alert inhibits MEDIUM alert for same metric).

**Throttling**: Repeat notifications every 4 hours (CRITICAL), 12 hours (HIGH), 24 hours (MEDIUM) until resolved.

### Alertmanager Configuration

```yaml
global:
  resolve_timeout: 5m

route:
  group_by: ["alertname", "severity"]
  group_wait: 30s
  group_interval: 5m
  repeat_interval: 4h
  receiver: "default"
  routes:
    - match:
        severity: critical
      receiver: "pagerduty"
      repeat_interval: 4h
    - match:
        severity: high
      receiver: "slack-email"
      repeat_interval: 12h
    - match:
        severity: medium
      receiver: "email-digest"
      repeat_interval: 24h

receivers:
  - name: "pagerduty"
    pagerduty_configs:
      - service_key: "<pagerduty_integration_key>"
  - name: "slack-email"
    slack_configs:
      - api_url: "<slack_webhook_url>"
        channel: "#alerts"
    email_configs:
      - to: "team@trapperkeeper.io"
  - name: "email-digest"
    email_configs:
      - to: "team@trapperkeeper.io"
        send_resolved: true
```

## Incident Response Procedures

### Incident Response Workflow

1. **Alert Received**: On-call engineer receives alert via configured destination
2. **Acknowledge**: Acknowledge alert in PagerDuty/Slack (prevents duplicate pages)
3. **Investigate**: Follow runbook for specific alert type
4. **Mitigate**: Apply immediate mitigation (restart service, clear cache, rollback deployment)
5. **Resolve**: Confirm alert resolved, update incident tracking
6. **Post-Mortem**: Conduct RCA for CRITICAL and HIGH alerts (within 48 hours)

### Runbooks

Each alert includes runbook URL with step-by-step troubleshooting instructions.

**Runbook Template**:

```markdown
# Runbook: [Alert Name]

## Overview

- Alert severity: [CRITICAL/HIGH/MEDIUM]
- Affected component: [Component name]
- Typical cause: [Common root causes]

## Immediate Actions

1. [First diagnostic step]
2. [Second diagnostic step]
3. [Immediate mitigation if issue confirmed]

## Diagnostic Queries

- [PromQL query for investigation]
- [Log query for investigation]
- [Database query if applicable]

## Common Resolutions

- [Resolution for cause 1]
- [Resolution for cause 2]
- [When to escalate]

## Escalation

- Escalate to: [Team/Person]
- Escalation threshold: [Time or condition]
- Contact: [Email/Slack/Phone]
```

### Post-Mortem Template

For CRITICAL and HIGH alerts requiring post-mortem analysis.

**Required Sections**:

1. **Incident Summary**: What happened, when, duration, impact
2. **Root Cause**: Technical root cause with evidence
3. **Timeline**: Key events from detection to resolution
4. **Resolution**: How incident was resolved
5. **Action Items**: Preventive measures to avoid recurrence
6. **Lessons Learned**: What worked well, what didn't

## Grafana Dashboard Integration

### Recommended Dashboards

**System Health Overview**:

- Error rate by category (time series)
- Request duration p50/p95/p99 (time series)
- Database connection pool utilization (gauge)
- Network consecutive failures (heatmap)

**Error Analysis**:

- Error rate by endpoint (time series)
- Validation errors by field (bar chart)
- 4xx vs 5xx error ratio (pie chart)
- Top errors by frequency (table)

**Performance Analysis**:

- Request latency distribution (histogram)
- Rule evaluation duration by condition count (scatter plot)
- Throughput (events per second, requests per second)
- Resource utilization (CPU, memory, disk)

**Business Metrics**:

- Rules created per day (time series)
- Events processed per day (time series)
- Match rate trend (time series)
- Active users (time series)

### Dashboard Variables

Use Grafana template variables for dynamic filtering:

- `$endpoint`: API endpoint selector
- `$severity`: Log level selector (`error`, `warn`, `info`)
- `$category`: Error category selector
- `$time_range`: Time range selector (last 1h, 6h, 24h, 7d)

## Edge Cases and Limitations

**High Cardinality Metrics**:

- Risk: Metrics with high-cardinality labels (rule_id, user_id) can overwhelm Prometheus
- Mitigation: Use sampling for high-cardinality metrics, limit retention period
- Recommendation: Use logs for high-cardinality debugging, metrics for aggregation

**Alert Fatigue**:

- Risk: Too many alerts desensitize engineers to critical issues
- Mitigation: Tune thresholds based on production patterns, inhibit lower severity alerts
- Recommendation: Review alert frequency quarterly, disable noisy alerts

**Metrics Collection Overhead**:

- Risk: Excessive metrics collection adds CPU and memory overhead
- Mitigation: Disable detailed metrics in hot paths (per-condition evaluation)
- Recommendation: Profile metrics overhead in performance testing

**Split-Brain Scenarios**:

- Risk: Multiple instances of same service may expose conflicting metrics
- Mitigation: Use instance labels, aggregate with `sum()` or `avg()`
- Recommendation: Use service discovery (Consul, Kubernetes) for automatic instance tracking

## Related Documents

**Hub Document**:

- [README.md](README.md): Strategic overview of resilience architecture and error handling principles

**Related Spokes** (siblings in resilience hub):

- [error-taxonomy.md](error-taxonomy.md): Error categories determining alert thresholds and severity
- [logging-standards.md](logging-standards.md): Structured logging enabling log-based metrics extraction
- [failure-modes.md](failure-modes.md): Degradation strategies requiring monitoring visibility

**Dependencies** (foundational documents):

- Operational Endpoints: Health check and metrics exposition endpoints
- API Service: gRPC error codes and status tracking
- Web Framework: HTTP status codes and request metrics

**Extended by**:

- Observability Index: Complete observability strategy integrating monitoring, logging, and tracing
- Performance Model: Performance budget enforcement using latency metrics
