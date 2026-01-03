---
doc_type: spoke
hub_document: doc/01-principles/README.md
status: active
primary_category: architecture
title: Schema-Agnostic Architecture
tags:
  - schema
  - runtime-resolution
  - field-paths
  - agile-data
---

# Schema-Agnostic Architecture

## Context

This document explains why TrapperKeeper's server has no schema knowledge and how field path resolution works at runtime in SDKs.

**Hub Document**: See [Architectural Principles](README.md) for the complete set of foundational principles and how they interrelate.

## Core Principle

**The central server has zero understanding of data schemas.**

Rules operate on abstract field paths resolved at runtime by SDKs. No schema
registry, no pre-registration required. The system works equally well with
schemaless/agile data pipelines and structured data.

## Motivation

Customers deploy 15+ diverse dataset types (compressed JSON, CSV, Parquet with
500K-point waveforms). Requiring schema registration creates friction and
becomes stale. Schema-agnostic design aligns with modern agile data pipelines
where schemas evolve rapidly and coordination overhead is prohibitive.

## Design Implications

### Field Path Resolution

Rules reference fields using abstract paths (e.g., `user.email`, `metrics[0]`).
Resolution happens at runtime in the SDK layer, not on the server:

- Server stores rule expressions as opaque strings
- SDK parses data structures and resolves paths dynamically
- No server-side type checking or field validation
- Rule evaluation succeeds/fails based on runtime data shape

### No Schema Registry

Traditional data systems require pre-registration of schemas:

- Schema registry maintains versioned schemas
- Producers/consumers coordinate on schema versions
- Schema evolution requires migration coordination

TrapperKeeper eliminates this entirely:

- No central schema repository
- No version negotiation between sensor and server
- Sensors send whatever data structure they have
- Rules evaluate against whatever structure arrives

### Schemaless and Structured Data

The same rule can evaluate against different data formats:

**JSON event:**

```json
{ "user": { "email": "alice@example.com" }, "action": "login" }
```

**CSV event (parsed to structure):**

```json
{ "col_0": "alice@example.com", "col_1": "login" }
```

**Parquet event:**

```json
{ "user_email": "alice@example.com", "event_type": "login" }
```

Rule `user.email == "alice@example.com"` evaluates successfully only if the
field path exists in the runtime structure. Missing fields are handled per
`on_missing_field` configuration (default: skip).

## Benefits

1. **Deployment Simplicity**: No schema pre-registration workflow
2. **Server Statelessness**: Server has no schema state to manage
3. **Schema Evolution**: Field additions/removals require no coordination
4. **Pipeline Agility**: Data structure changes don't break existing rules (fail
   gracefully)
5. **Universal Application**: Same rule engine works for JSON, CSV, Parquet,
   Protocol Buffers

## Tradeoffs

1. **Limited Validation**: Server cannot validate field existence or types
   before rule deployment
2. **Runtime Errors**: Type mismatches discovered during evaluation, not at rule
   creation
3. **Field Name Brittleness**: Typos in field paths (e.g., `usr.email` vs
   `user.email`) not caught until runtime
4. **No Schema Documentation**: No central place documenting available fields
5. **Client Dependency**: SDKs must implement field resolution correctly; server
   cannot verify

## Implementation

### Server-Side (tk-api-server)

Rules stored as text in database:

```sql
CREATE TABLE rules (
  rule_id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  expression TEXT NOT NULL  -- Opaque string: "user.email == 'alice@example.com'"
);
```

No schema parsing, no field validation, no type inference. Expression is opaque
to server.

### SDK-Side (tk-sdk)

SDKs parse rule expressions and resolve field paths against runtime data
structures:

```go
// Pseudocode: SDK evaluates rule
func (s *Sensor) evaluateRule(rule Rule, data map[string]interface{}) bool {
    expr := parseExpression(rule.Expression)  // "user.email == 'alice@example.com'"
    leftValue := resolvePath(data, expr.Left) // Navigate data["user"]["email"]
    rightValue := expr.Right                  // "alice@example.com"
    return compare(leftValue, rightValue, expr.Op)
}
```

Field path resolution is SDK responsibility. Server has no visibility into data
structures.

### Error Handling

When field path resolution fails (field doesn't exist):

- Default behavior: Skip rule (rule treated as not-matching)
- Configurable via `on_missing_field`:
  - `skip`: Treat as non-match
  - `error`: Treat as evaluation error (log warning)
  - `fail_closed`: Treat as match (conservative for security rules)

## Cross-References

- [Rule Expression Language](../04-rule-engine/expression-language.md) - Field
  path syntax and operators
- [Field Path Resolution](../04-rule-engine/field-path-resolution.md) - SDK-side
  resolution algorithm
- [SDK Model](../02-architecture/sdk-model.md) - SDK operates on parsed
  structures, not wire formats
- [Type System and Coercion](../04-rule-engine/type-system-coercion.md) -
  Runtime type handling
- [Schema Evolution](../04-rule-engine/schema-evolution.md) - Handling schema
  changes without coordination

## Future Considerations

### Optional Schema Registry

For customers requiring stricter validation:

- Optional schema registration endpoint
- Server validates rules against registered schemas at creation time
- Fail early on typos/type mismatches
- Backward compatible: schema registration is opt-in, not required

### Schema Inference

For operational visibility:

- SDK could report observed field paths back to server
- Server aggregates common schemas across sensors
- Provides documentation/autocomplete without requiring registration
- Read-only inference, not enforcement
