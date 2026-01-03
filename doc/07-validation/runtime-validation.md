---
doc_type: spoke
status: active
primary_category: validation
hub_document: doc/07-validation/README.md
tags:
  - runtime-validation
  - rule-evaluation
  - sdk
---

# Runtime Layer Validation

## Context

Runtime layer enforces validation during rule evaluation in both SDK execution (sdks/go) and server processing (cmd/sensor-api, cmd/web-ui). Different validation types apply to different scopes (SDK-only, server-only, or both). Validation logic centralized in internal/rules package ensures consistency across all execution contexts.

**Hub Document**: This document is part of the Validation Architecture. See [Validation Hub](README.md) for complete validation strategy and layer distribution.

## Validation Scope (SDK vs Server)

Runtime validation applies to three scopes based on validation type.

### Scope Matrix

| Validation Type    | SDK (tk-client) | Server (tk-core) | Notes                                                  |
| ------------------ | --------------- | ---------------- | ------------------------------------------------------ |
| Structural Format  | ✓               | ✓                | Format validation during deserialization               |
| Rule Expression    | ✓               | ✓                | Constraint enforcement during rule evaluation          |
| Type Coercion      | ✓               | ✓                | Coercion per type system matrix                        |
| Resource Limits    | ✓               | ✗                | Buffer limits enforced only in SDK before transmission |
| Field Resolution   | ✓               | ✓                | Missing field detection, on_missing_field policy       |
| Performance/Cost   | ✓               | ✓                | Sampling rate enforcement                              |
| Input Sanitization | ✓               | ✓                | Defense-in-depth re-validation                         |

**Cross-References**:

- Client/Server Separation: Package boundaries and validation ownership
- Validation Hub: Scope matrix (see table above)

## Type Coercion Validation

Type coercion failures treated as condition failed (NOT missing field).

### Coercion Behavior

```go
// Missing field: Apply on_missing_field policy
if fieldValue == nil {
    switch condition.OnMissingField {
    case OnMissingFieldSkip:
        return EvalResultSkip
    case OnMissingFieldFail:
        return EvalResultNoMatch
    case OnMissingFieldMatch:
        return EvalResultMatch
    }
}

// Type coercion failure: Condition failed (NOT missing field)
coercedValue, err := coerceNumeric(fieldValue)
if err != nil {
    slog.Debug("Type coercion failed", "error", err)
    return EvalResultNoMatch  // Condition failed, continue evaluation
}
```

**Critical Distinction**:

- **Null values** (e.g., `pd.NA`, `None`) → Treated as missing field, defer to on_missing_field policy
- **Coercion failures** (e.g., `"invalid"` → numeric) → Condition fails, continue evaluation

**Cross-References**:

- Type System: Complete coercion matrix
- Schema Evolution: Missing field handling

## Field Resolution Validation

Missing field detection with on_missing_field policy enforcement.

### Policy Application

**Three policies**:

- **skip** (default): Rule doesn't match, continue evaluation
- **fail**: Mark rule as failed
- **match**: Treat missing field as successful match

```go
slog.Debug(
    "Field not found, skipping condition per policy",
    "target", "rule_evaluation",
    "rule_id", rule.ID,
    "field", fieldPath,
    "policy", "skip",
)
```

**Cross-References**:

- Field Path Resolution: Field path semantics
- Schema Evolution: on_missing_field policy specification

## Resource Limit Enforcement (SDK Only)

Buffer limits enforced in SDK before transmission. Server receives unbuffered events via gRPC.

### Buffer Limits

- 128 events (configurable)
- 1MB per event
- 128MB total memory cap

**Auto-flush**: SDK automatically flushes when buffer reaches limits.

```go
if len(buffer.events) >= MaxEvents || buffer.totalSize >= MaxSize {
    if err := flushEvents(&buffer); err != nil {
        return err
    }
}
```

**Cross-References**:

- SDK Model: Complete buffer management specification
- Client Metadata: Metadata size limits

## Logging Standards

Structured logging with Go slog package.

### Debug Mode vs Release Mode

**Debug mode**: Validate assumptions frequently; log verbose diagnostics

```go
slog.Debug(
    "Type coercion failed, treating as condition failed",
    "target", "rule_evaluation",
    "rule_id", rule.ID,
    "field", fieldPath,
    "value_type", actualType,
    "expected_type", expectedType,
)
```

**Release mode**: Validate once when data loaded; optimize for performance

**Cross-References**:

- Resilience Hub: Complete logging standards and structured format

## Related Documents

**Dependencies**: Validation Hub, Type System, Field Path Resolution, Schema Evolution

**Related Spokes**:

- Responsibility Matrix: Complete Runtime Layer validation assignments with explicit SDK vs Server scope markers
- API Validation: API validation occurs before runtime validation
- UI Validation: UI validation provides immediate feedback before API/runtime validation
- Database Validation: Database constraints complement runtime validation
