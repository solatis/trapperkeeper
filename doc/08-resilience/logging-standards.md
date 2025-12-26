---
doc_type: spoke
status: active
date_created: 2025-11-10
primary_category: observability
hub_document: doc/08-resilience/README.md
tags:
  - logging
  - observability
  - distributed-tracing
  - structured-logging
  - slog
  - opentelemetry
---

# Logging Standards

## Context

Comprehensive, structured logging is essential for debugging distributed systems, monitoring production health, and conducting incident response. Without consistent logging standards, debugging becomes difficult due to inconsistent field names, missing context, and inappropriate log levels.

This document establishes logging standards for TrapperKeeper using Go's `slog` package (stdlib Go 1.21+), including log level guidelines, structured logging format with required fields, distributed tracing patterns with OpenTelemetry integration, and critical security requirements for log sanitization.

**Hub Document**: This document is part of the Resilience Architecture hub. See [README.md](README.md) for strategic overview of error handling, error taxonomy, and monitoring strategy integration.

## Framework: Go slog Package

**ABSOLUTE REQUIREMENT**: Use Go's `slog` package (stdlib Go 1.21+) exclusively for all logging.

**FORBIDDEN**: NEVER use:

- `log` package (legacy, unstructured)
- `fmt.Println` or `fmt.Fprintf` to stderr (bypasses structured logging)
- Custom logging libraries (breaks consistency)

**Rationale**: The `slog` package provides structured logging with context propagation, enabling distributed tracing integration via OpenTelemetry and consistent field extraction for log aggregation systems (ELK, Loki, CloudWatch).

**Dependencies**:

```go
import (
    "log/slog"
    "os"
)
```

**Initialization** (service startup):

```go
func initLogging() {
    // JSON handler for production (structured output)
    handler := slog.NewJSONHandler(os.Stdout, &slog.HandlerOptions{
        Level: slog.LevelInfo,
    })
    logger := slog.New(handler)
    slog.SetDefault(logger)
}
```

## Log Levels and Usage

Log levels follow standard severity hierarchy with specific semantic meanings for TrapperKeeper operations.

### ERROR: System-Level Failures

**Definition**: System-level failures requiring immediate operator intervention.

**Examples**:

- Database errors (connection failure, constraint violation, disk full)
- Service crashes (panic, unrecoverable error)
- Configuration errors (missing required config, invalid format)
- Critical resource exhaustion (out of memory, file descriptors)

**Required Context**:

- `error`: Error details (type, message, backtrace if available)
- `operation`: Operation that failed (e.g., "database_query", "service_startup")
- Operation-specific fields: query description, endpoint, file path, etc.

**Example**:

```go
slog.Error("Database query failed",
    "target", "database",
    "error", err,
    "query", queryDescription,  // Sanitized, no actual values
    "operation", "query",
)
```

**Monitoring Integration**: ERROR logs trigger CRITICAL or HIGH severity alerts requiring immediate attention.

**When to Use**:

- Any database error (fail-fast pattern, no retry)
- Service initialization failures (cannot start)
- Configuration loading failures (invalid config file)
- Unrecoverable network errors (after retry exhaustion)

**When NOT to Use**:

- Expected errors in schema-agnostic system (type coercion, missing fields)
- Client input errors (validation failures, malformed requests)
- Transient network errors (before retry exhaustion)

### WARN: Degraded Functionality

**Definition**: Degraded functionality or potential issues not requiring immediate intervention but indicating system stress or configuration problems.

**Examples**:

- Network failures before alert threshold (first 5 consecutive failures)
- Missing field with fail policy (expected in some schemas)
- Deprecated API usage (scheduled for removal)
- Resource usage approaching limits (80% memory, 90% disk)

**Required Context**:

- `operation`: Degraded operation
- `degradation_scope`: What functionality is affected
- Operation-specific context: endpoint, resource type, etc.

**Example**:

```go
slog.Warn("Network failure, will retry",
    "target", "network",
    "endpoint", endpointURL,
    "error", err,
    "retry_attempt", attempt,
    "max_retries", maxAttempts,
)
```

**Monitoring Integration**: WARN logs contribute to error rate metrics and may trigger MEDIUM severity alerts if sustained.

**When to Use**:

- Network errors during retry backoff (before exhaustion)
- Missing field when rule has fail policy
- Deprecated feature usage
- Protocol errors (client 4xx errors at moderate rates)

**When NOT to Use**:

- Normal operational events (use INFO)
- Debug diagnostics (use DEBUG)
- Critical failures (use ERROR)

### INFO: Normal Operational Events

**Definition**: Normal operational events indicating system state transitions and important business events.

**Examples**:

- Service startup/shutdown
- Configuration changes (reload, hot-swap)
- Validation errors (client input errors)
- User authentication events (login, logout)
- API request completion (high-level, not per-field)

**Required Context**:

- Event type identifier
- Relevant IDs (user_id, request_id, session_id)
- Duration for timed operations

**Example**:

```go
slog.Info("Request completed",
    "target", "api",
    "request_id", requestID,
    "user_id", user.ID,
    "endpoint", endpointPath,
    "duration_ms", duration.Milliseconds(),
    "status_code", status,
)
```

**Monitoring Integration**: INFO logs provide audit trail and operational visibility without noise.

**When to Use**:

- Service lifecycle events (start, stop, reload)
- Validation errors (client responsibility, not system error)
- Authentication events (security audit trail)
- Configuration changes
- High-level request completion (not per-operation)

**When NOT to Use**:

- Detailed diagnostics (use DEBUG)
- Normal internal operations (use DEBUG or omit)
- Error conditions (use ERROR or WARN)

### DEBUG: Detailed Diagnostic Information

**Definition**: Detailed diagnostic information for debugging and troubleshooting, typically disabled in production.

**Examples**:

- Type coercion failures (expected in schema-agnostic system)
- Missing fields with skip policy (expected behavior)
- Rule evaluation details (condition results, field lookups)
- Internal state transitions (cache hit/miss, rule compilation)

**Required Context**:

- Operation identifiers (rule_id, request_id)
- Field paths and values (sanitized)
- Expected vs actual values (for diagnostics)

**Example**:

```go
slog.Debug("Type coercion failed, treating as condition failed",
    "target", "rule_evaluation",
    "rule_id", rule.ID,
    "field", fieldPath,
    "value_type", actualType,
    "expected_type", expectedType,
)
```

**Monitoring Integration**: DEBUG logs not aggregated for alerting. May be sampled for performance analysis.

**When to Use**:

- Type coercion failures (Category 2 errors)
- Missing fields with skip policy (Category 3 errors)
- Rule evaluation diagnostics
- Cache operations (hit/miss/eviction)
- Field path resolution steps

**When NOT to Use**:

- Very detailed internal operations (use TRACE)
- Normal operational events (use INFO)
- Error conditions requiring action (use WARN or ERROR)

### DEBUG-1: Very Detailed Diagnostic

**Definition**: Very detailed diagnostic information for deep troubleshooting, typically disabled even in development unless debugging specific issues.

**Note**: Go's `slog` package uses DEBUG level for detailed diagnostics. For very detailed tracing, use DEBUG with additional filtering via `slog.HandlerOptions.Level` set to a custom level or enable only for specific loggers.

**Examples**:

- Per-condition evaluation in multi-condition rules
- Field lookup paths with wildcard expansion
- Internal state transitions for complex algorithms
- Performance instrumentation (span timings)

**Required Context**:

- Span context (trace_id, span_id, parent_span_id)
- Step identifiers (condition_index, iteration_count)
- Detailed internal state

**Example**:

```go
slog.Debug("Evaluating condition",
    "target", "rule_evaluation",
    "rule_id", rule.ID,
    "condition_index", idx,
    "total_conditions", len(conditions),
    "field_path", condition.Field,
)
```

**Monitoring Integration**: Very detailed DEBUG logs never aggregated. Used only for local debugging with OpenTelemetry tracing enabled.

**When to Use**:

- Per-condition evaluation in rules
- Wildcard field path expansion steps
- Loop iterations in complex algorithms
- Span timing instrumentation

**When NOT to Use**:

- Any production scenario (too verbose)
- General debugging (use DEBUG)
- Operational visibility (use INFO)

## Structured Logging Format

All log entries use structured field syntax with consistent field names and formatting.

### Field Formatting Patterns

**Key-Value Pairs**: slog uses variadic key-value pairs for structured fields

```go
slog.Error("Database operation failed",
    "target", "database",
    "error", err,              // Error value
    "operation", "query",      // String literal
    "retry_count", count,      // Numeric value
)
```

**Complex Types**: Use fmt.Sprintf or custom formatting for complex types

```go
slog.Error("Database query failed",
    "target", "database",
    "query", queryDescription.String(),  // Complex struct with String() method
    "params", fmt.Sprintf("%v", paramValues),  // Slice/array
)
```

**Structured Attributes**: Group related fields using slog.Group

```go
slog.Info("API request received",
    "target", "api",
    slog.Group("request",
        "endpoint", "/api/rules",
        "method", "POST",
    ),
)
```

### Required Fields by Target

Each logging target has specific required fields for consistency and debuggability.

#### Target: `database`

**Purpose**: Database operations (queries, transactions, migrations)

**Required Fields**:

- `error`: Error details
- `operation`: Operation type (`query`, `insert`, `update`, `delete`, `transaction`, `migration`)
- `query`: Query description (sanitized, parameterized)

**Optional Fields**:

- `duration_ms`: Query execution time
- `affected_rows`: Number of rows affected
- `table`: Primary table name

**Example**:

```go
slog.Error("Database query failed",
    "target", "database",
    "error", err,
    "operation", "query",
    "query", "SELECT * FROM rules WHERE user_id = ?",
    "table", "rules",
)
```

#### Target: `network`

**Purpose**: Network communication (HTTP clients, gRPC clients, external APIs)

**Required Fields**:

- `error`: Error details
- `endpoint`: Target endpoint URL or hostname
- `retry_attempt`: Current retry attempt (0-based)

**Optional Fields**:

- `timeout_ms`: Timeout value
- `method`: HTTP method or gRPC method name
- `status_code`: HTTP or gRPC status code

**Example**:

```go
slog.Error("Network request failed",
    "target", "network",
    "error", err,
    "endpoint", "https://api.trapperkeeper.io/sync",
    "retry_attempt", attempt,
    "timeout_ms", timeout.Milliseconds(),
)
```

#### Target: `rule_evaluation`

**Purpose**: Rule engine evaluation logic

**Required Fields**:

- `rule_id`: Rule UUID
- `field`: Field path (if applicable)

**Optional Fields**:

- `value_type`: Actual value type (for coercion diagnostics)
- `expected_type`: Expected value type (for coercion diagnostics)
- `policy`: Policy applied (for missing field handling)
- `condition_index`: Condition index in multi-condition rules

**Example**:

```go
slog.Debug("Type coercion failed, treating as condition failed",
    "target", "rule_evaluation",
    "rule_id", rule.ID,
    "field", fieldPath,
    "value_type", "string",
    "expected_type", "number",
)
```

#### Target: `validation`

**Purpose**: Input validation and sanitization

**Required Fields**:

- `request_id`: Request identifier
- `field`: Field that failed validation (JSON path notation)
- `validation_error`: Validation error type

**Optional Fields**:

- `user_input`: Sanitized user input (truncated, no PII)
- `constraint`: Validation constraint violated
- `expected_format`: Expected format (for format validation)

**Example**:

```go
slog.Info("Validation failed for user input",
    "target", "validation",
    "request_id", requestID,
    "field", "conditions[0].operator",
    "validation_error", "invalid_operator",
    "user_input", sanitizedInput,
)
```

#### Target: `api`

**Purpose**: API request processing (HTTP, gRPC)

**Required Fields**:

- `request_id`: Request identifier
- `endpoint`: API endpoint path or gRPC method
- `duration_ms`: Request duration

**Optional Fields**:

- `user_id`: Authenticated user ID (if available)
- `status_code`: HTTP or gRPC status code
- `method`: HTTP method
- `client_ip`: Client IP address (sanitized, respecting privacy)

**Example**:

```go
slog.Info("Request completed successfully",
    "target", "api",
    "request_id", requestID,
    "endpoint", "/api/rules",
    "method", "POST",
    "duration_ms", duration.Milliseconds(),
    "status_code", 201,
    "user_id", user.ID,
)
```

#### Target: `protocol`

**Purpose**: Protocol-level errors (HTTP, gRPC, serialization)

**Required Fields**:

- `request_id`: Request identifier
- `error_code`: Protocol error code (HTTP status, gRPC code)
- `error`: Error details

**Optional Fields**:

- `client_info`: Client metadata (user agent, SDK version)
- `endpoint`: Target endpoint
- `method`: HTTP method or gRPC method

**Example**:

```go
slog.Warn("Malformed request",
    "target", "protocol",
    "request_id", requestID,
    "error_code", 400,
    "error", err,
    "endpoint", "/api/rules",
)
```

## Distributed Tracing

Distributed tracing enables request tracking across service boundaries using context propagation and OpenTelemetry integration.

### Context Propagation with context.Context

**Context Components**:

- `trace_id`: Unique identifier for entire request flow (extracted from OpenTelemetry span)
- `span_id`: Unique identifier for current operation
- `parent_span_id`: Parent span identifier (for nested operations)

**Context Propagation Mechanisms**:

**HTTP Headers** (for HTTP/REST APIs):

```go
import (
    "context"
    "net/http"
    "github.com/google/uuid"
)

// Sender side (outgoing request)
traceID := uuid.New()
spanID := uuid.New()

req, _ := http.NewRequestWithContext(ctx, "POST", url, body)
req.Header.Set("X-Trace-Id", traceID.String())
req.Header.Set("X-Span-Id", spanID.String())

client := &http.Client{}
resp, err := client.Do(req)

// Receiver side (incoming request)
traceID, err := uuid.Parse(r.Header.Get("X-Trace-Id"))
if err != nil {
    traceID = uuid.New()
}

// Attach to context for downstream propagation
ctx := context.WithValue(r.Context(), "trace_id", traceID)
```

**gRPC Metadata** (for gRPC APIs):

```go
import (
    "context"
    "google.golang.org/grpc/metadata"
)

// Sender side (outgoing gRPC call)
md := metadata.Pairs(
    "x-trace-id", traceID.String(),
    "x-span-id", spanID.String(),
)
ctx := metadata.NewOutgoingContext(ctx, md)

// Receiver side (gRPC service method)
md, ok := metadata.FromIncomingContext(ctx)
if ok {
    if vals := md.Get("x-trace-id"); len(vals) > 0 {
        traceID, _ = uuid.Parse(vals[0])
    }
}
```

### Span Creation and Nesting

**OpenTelemetry Integration** (recommended for production):

```go
import (
    "context"
    "log/slog"
    "go.opentelemetry.io/otel"
    "go.opentelemetry.io/otel/trace"
)

// Top-Level Span (per-request)
func handleRequest(ctx context.Context, request Request) (Response, error) {
    tracer := otel.Tracer("trapperkeeper")
    ctx, span := tracer.Start(ctx, "handle_request",
        trace.WithAttributes(
            attribute.String("request_id", request.ID.String()),
            attribute.String("endpoint", request.Path),
        ),
    )
    defer span.End()

    // Logger with trace context
    logger := slog.With(
        "trace_id", span.SpanContext().TraceID().String(),
        "span_id", span.SpanContext().SpanID().String(),
    )

    logger.Info("Request received", "endpoint", request.Path)

    result, err := processRequest(ctx, request)
    if err != nil {
        logger.Error("Request processing failed", "error", err)
        return Response{}, err
    }

    return result, nil
}
```

**Nested Spans** (sub-operations with slog.With):

```go
func processRequest(ctx context.Context, request Request) (Response, error) {
    tracer := otel.Tracer("trapperkeeper")

    // Database query span
    ctx, dbSpan := tracer.Start(ctx, "database_query")
    logger := slog.With(
        "operation", "query_rules",
        "user_id", request.UserID.String(),
    )
    rules, err := db.QueryRules(ctx, request.UserID)
    dbSpan.End()
    if err != nil {
        logger.Error("Database query failed", "error", err)
        return Response{}, err
    }

    // Rule evaluation span
    ctx, evalSpan := tracer.Start(ctx, "rule_evaluation")
    logger = slog.With("rule_count", len(rules))
    results, err := evaluateRules(ctx, rules, request.Event)
    evalSpan.End()
    if err != nil {
        logger.Error("Rule evaluation failed", "error", err)
        return Response{}, err
    }

    return Response{Results: results}, nil
}
```

**Span Timing with Deferred Logging**:

Manual span timing using defer pattern with slog.With:

```go
func queryUserRules(ctx context.Context, db *Database, userID uuid.UUID) ([]Rule, error) {
    start := time.Now()
    logger := slog.With("user_id", userID.String())

    defer func() {
        logger.Info("Query completed", "duration_ms", time.Since(start).Milliseconds())
    }()

    return db.Query(ctx, "SELECT * FROM rules WHERE user_id = ?", userID)
}
```

## Log Sanitization (CRITICAL SECURITY)

Log sanitization prevents sensitive data leakage and log injection attacks.

### Sensitive Data (NEVER LOG)

**Secrets and Credentials**:

- Passwords (plaintext, hashed, bcrypt)
- API secrets and HMAC keys
- Session tokens and JWT tokens
- TLS private keys
- Database connection strings with passwords

**Example** (FORBIDDEN):

```go
// WRONG - logs password
slog.Error("Login failed", "username", username, "password", password)

// CORRECT - omits password
slog.Error("Login failed", "user_id", user.ID)
```

### SQL Query Sanitization

**NEVER log interpolated SQL queries**. Always log parameterized queries with placeholders.

**Example** (FORBIDDEN):

```go
// WRONG - logs actual values (security risk, SQL injection in logs)
query := fmt.Sprintf("SELECT * FROM users WHERE email = '%s'", email)
slog.Error("Database query failed", "query", query)

// CORRECT - logs parameterized query
slog.Error("Database query failed",
    "query", "SELECT * FROM users WHERE email = ?",
    "params", []any{email},  // Separate parameters
)
```

**Rationale**: Interpolated queries expose sensitive data in logs and can reveal SQL injection attack patterns.

### User Input Sanitization

**Escape control characters** to prevent log injection attacks.

**Example**:

```go
func sanitizeForLogging(input string) string {
    var result strings.Builder
    for _, r := range input {
        switch r {
        case '\n':
            result.WriteString("\\n")
        case '\r':
            result.WriteString("\\r")
        case '\t':
            result.WriteString("\\t")
        default:
            if unicode.IsControl(r) {
                fmt.Fprintf(&result, "\\x%02x", r)
            } else {
                result.WriteRune(r)
            }
        }
    }
    return result.String()
}

// Usage
sanitized := sanitizeForLogging(userInput)
slog.Info("Validation failed",
    "target", "validation",
    "field", "rule_name",
    "user_input", sanitized,
)
```

**Truncate long inputs** to prevent log volume attacks:

```go
func truncateForLogging(input string, maxLen int) string {
    if len(input) <= maxLen {
        return input
    }
    return fmt.Sprintf("%s... (truncated %d chars)", input[:maxLen], len(input)-maxLen)
}
```

### PII Sanitization (Future Consideration)

**Current Status**: PII sanitization not required for MVP (no PII collection in event data).

**Future Requirements**:

- Sanitize email addresses (replace with hash or `user_***@domain.com`)
- Sanitize IP addresses (mask last octet: `192.168.1.***`)
- Sanitize phone numbers (mask middle digits: `+1-***-***-1234`)
- Sanitize credit card numbers (show last 4 digits: `****-****-****-1234`)

**Regulatory Context**: GDPR, CCPA, HIPAA may require PII sanitization in logs depending on customer requirements.

## Log Aggregation and Storage

### Log Format (JSON)

Structured logs output JSON format for ingestion by log aggregation systems.

**Configuration**:

```go
import (
    "log/slog"
    "os"
)

func initJSONLogging() {
    handler := slog.NewJSONHandler(os.Stdout, &slog.HandlerOptions{
        Level: slog.LevelInfo,
        AddSource: true,  // Include source file/line information
    })
    logger := slog.New(handler)
    slog.SetDefault(logger)
}
```

**Example Output**:

```json
{
  "timestamp": "2025-11-10T15:32:45.123Z",
  "level": "ERROR",
  "target": "database",
  "message": "Database query failed",
  "fields": {
    "error": "connection timeout",
    "operation": "query",
    "query": "SELECT * FROM rules WHERE user_id = ?",
    "table": "rules"
  },
  "span": {
    "trace_id": "01936a3e-1234-7b3c-9d5e-abcdef123456",
    "span_id": "01936a3e-5678-7b3c-9d5e-abcdef789012",
    "name": "database_query"
  }
}
```

### Log Retention

**Development**: 7 days (local disk, Docker volumes)
**Staging**: 30 days (cloud storage)
**Production**: 90 days (cloud storage with archival to cold storage)

**Rationale**: Balances debugging needs, compliance requirements, and storage costs.

### Log Aggregation Systems

**Recommended Systems**:

- **ELK Stack** (Elasticsearch, Logstash, Kibana): Self-hosted, full-featured
- **Grafana Loki**: Lightweight, cost-effective, integrates with Grafana
- **AWS CloudWatch Logs**: Managed, AWS-native
- **Google Cloud Logging**: Managed, GCP-native

**Integration Pattern**:

Logs output to stdout in JSON format. Container runtime or log shipper (Fluentd, Vector, CloudWatch Agent) forwards to aggregation system.

## Edge Cases and Limitations

**High-Volume Logging**:

- Risk: Excessive DEBUG/TRACE logging can overwhelm storage
- Mitigation: Use sampling for high-frequency operations (per-condition evaluation)
- Recommendation: Enable DEBUG only for specific modules in production

**Log Injection Attacks**:

- Risk: Unsanitized user input can inject newlines and control characters
- Mitigation: Sanitize all user input with `sanitize_for_logging` function
- Validation: Log aggregation systems may provide additional filtering

**Sensitive Data Leakage**:

- Risk: Accidentally logging passwords, API keys, or PII
- Mitigation: Code review, security scanning tools, sanitization libraries
- Recommendation: Use structured logging (field syntax) to avoid string interpolation

**Span Context Overhead**:

- Risk: Span creation/destruction adds CPU and memory overhead
- Mitigation: Use spans for high-level operations (request, database query), not low-level operations (field lookup)
- Recommendation: Profile span overhead in performance testing

## Related Documents

**Hub Document**:

- [README.md](README.md): Strategic overview of resilience architecture and error handling principles

**Related Spokes** (siblings in resilience hub):

- [error-taxonomy.md](error-taxonomy.md): Error categories determining log levels and required fields
- [monitoring-strategy.md](monitoring-strategy.md): Alert thresholds using log aggregation metrics
- [failure-modes.md](failure-modes.md): Degradation strategies requiring specific logging patterns

**Dependencies** (foundational documents):

- Architectural Principles: Simplicity principle (minimal logging overhead)
- API Service: gRPC context propagation patterns
- Web Framework: HTTP request logging patterns

**Extended by**:

- Security Architecture: Log sanitization requirements for compliance
- Observability Index: Complete observability strategy integrating logging, metrics, and tracing
