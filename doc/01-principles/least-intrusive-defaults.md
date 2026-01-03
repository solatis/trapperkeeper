---
doc_type: spoke
hub_document: doc/01-principles/README.md
status: active
primary_category: architecture
title: Least Intrusive by Default
tags:
  - failure-modes
  - fail-safe
  - degradation
  - observability
---

# Least Intrusive by Default

## Core Principle

**System degrades to pass-through rather than failing pipelines.**

TrapperKeeper is an observability layer, not core business logic. Production
pipelines must not fail because rule evaluation has issues. Default behavior
prioritizes pipeline continuity over strict correctness.

## Motivation

Observability tools should be invisible when working correctly and non-blocking
when broken. A monitoring system that crashes production pipelines is worse than
no monitoring at all.

Five-engineer startup cannot afford 24/7 on-call for observability
infrastructure. System must degrade gracefully without manual intervention.

## Default Behaviors

### Network Failures

When SDK cannot reach TrapperKeeper server:

**Fail-Safe Mode** (default):

- Disable all rules
- Sensor becomes no-op (pass-through)
- Pipeline continues processing data
- Log connection failure warnings

**Configurable alternatives:**

- `fail_closed`: Cache last-known rules, continue evaluating (stale rules better
  than no monitoring)
- `fail_strict`: Block pipeline until server reachable (for critical compliance
  scenarios)

### Missing Fields

When rule references field that doesn't exist in data:

**Skip Rule** (default: `on_missing_field="skip"`):

- Treat rule as non-matching
- Continue evaluating other rules
- No error logged (field absence is normal in schema-agnostic system)

**Configurable alternatives:**

- `on_missing_field="error"`: Log warning, treat as evaluation error
- `on_missing_field="fail_closed"`: Treat as match (conservative for security
  rules)

### Type Coercion Failures

When field value cannot be coerced to expected type:

**Treat as Condition Failed** (default):

- Rule condition evaluates to false
- Continue evaluating other rules
- No error logged (type mismatches expected in dynamic data)

**Example:**

```
Rule: user_id > 1000
Data: {"user_id": "alice"}  // String, not number
Result: Condition fails (cannot coerce "alice" to number for comparison)
```

### Event POST Failures

When SDK cannot POST event to server:

**Log Warning, Continue Processing** (default):

- Event data lost (no local persistence)
- Pipeline continues
- Warning logged for operational visibility

**Configurable alternatives:**

- `buffer_events=true`: Buffer events in-memory, retry on reconnect (bounded
  buffer to prevent OOM)
- `fail_on_event_loss=true`: Block pipeline if events cannot be delivered
  (critical audit scenarios)

## Operational Modes

### Development Mode

Stricter defaults for catching issues early:

- `on_missing_field="error"` (surface field name typos)
- `fail_on_event_loss=true` (ensure events captured during testing)
- Verbose logging enabled

### Production Mode

Least intrusive defaults for pipeline stability:

- `on_missing_field="skip"` (tolerate schema variations)
- `fail_safe=true` (network failures disable rules)
- Minimal logging (warn-level and above)

## Benefits

1. **Pipeline Safety**: Observability issues never break production data flows
2. **Operational Simplicity**: No emergency on-call for TrapperKeeper outages
3. **Schema Tolerance**: Missing fields don't crash rule evaluation
4. **Gradual Rollout**: Deploy TrapperKeeper without fear of breaking existing
   systems
5. **Debug/Prod Parity**: Same codebase, different config for dev strictness vs
   prod safety

## Tradeoffs

1. **Error Visibility**: Silent failures in fail-safe mode may hide
   misconfigurations
2. **Event Loss**: Network failures can lose observability data
3. **Stale Rules**: Fail-closed mode evaluates outdated rules during network
   partitions
4. **Type Safety**: Runtime type mismatches discovered late
5. **Debug Complexity**: Silent skips harder to troubleshoot than loud failures

## Implementation

### SDK Configuration

```go
type SensorConfig struct {
    // Network failure behavior
    FailureMode FailureMode  // FailSafe (default) | FailClosed | FailStrict

    // Missing field behavior
    OnMissingField MissingFieldMode  // Skip (default) | Error | FailClosed

    // Event delivery
    BufferEvents bool  // false (default)
    BufferSize int     // Bounded buffer when BufferEvents=true

    // Logging
    LogLevel LogLevel  // Warn (default) | Info | Debug
}
```

### Fail-Safe State Machine

SDK maintains connection state:

```
[CONNECTED] --network error--> [FAIL_SAFE]
[FAIL_SAFE] --reconnect--> [CONNECTED]
[FAIL_SAFE] --evaluate()-- --> return NoMatch (all rules)
```

In `FAIL_SAFE` state:

- All rule evaluations return non-matching
- No event POSTs attempted
- Periodic reconnection attempts (exponential backoff)

### Missing Field Handling

Rule evaluation pseudocode:

```go
func evaluateCondition(data, fieldPath, expectedValue) bool {
    value, exists := resolvePath(data, fieldPath)
    if !exists {
        switch config.OnMissingField {
        case Skip:
            return false  // Rule doesn't match
        case Error:
            logWarn("Field not found: %s", fieldPath)
            return false
        case FailClosed:
            return true   // Conservative: treat as match
        }
    }
    return compare(value, expectedValue)
}
```

## Cross-References

- [Failure Modes and Degradation](../08-resilience/failure-modes.md) - Complete
  failure mode taxonomy and recovery strategies
- [Error Handling Strategy](../08-resilience/README.md) - Unified error patterns
  across system
- [SDK Model](../02-architecture/sdk-model.md) - SDK implements fail-safe state
  machine
- [Field Path Resolution](../04-rule-engine/field-path-resolution.md) - Missing
  field handling in path resolution
- [Type System and Coercion](../04-rule-engine/type-system-coercion.md) - Type
  coercion failure handling

## Future Considerations

### Observability Metrics

Track degradation events for operational visibility:

- Counter: `sensor_failsafe_events` (network failures triggering fail-safe mode)
- Counter: `rule_missing_field_skips` (rules skipped due to missing fields)
- Counter: `event_delivery_failures` (events lost due to POST failures)
- Histogram: `failsafe_duration_seconds` (time spent in fail-safe mode)

Metrics endpoint on SDK (Prometheus format) enables external monitoring without
server dependency.

### Circuit Breaker

Advanced network failure handling:

- Track failure rate over sliding window
- Open circuit after threshold (e.g., 5 failures in 60s)
- Half-open state for testing recovery
- Prevents thundering herd on server restart

### Partial Event Buffering

For high-value events:

- Tag rules as `critical=true`
- Buffer events from critical rules even when `BufferEvents=false`
- Bounded buffer per criticality level
- Balances memory usage with event importance
