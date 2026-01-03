---
doc_type: hub
status: active
primary_category: architecture
tags:
  - rule-engine
  - expression-language
  - dnf
  - evaluation
consolidated_spokes:
  - expression-language.md
  - field-path-resolution.md
  - type-system-coercion.md
  - schema-evolution.md
  - lifecycle.md
cross_cutting:
  - validation
  - performance
maintainer: Rule Engine Team
---

# Rule Engine Architecture

## Context

TrapperKeeper's rule engine evaluates data quality rules against streaming records with strict performance requirements (<1ms per record) while maintaining usability for non-programmers. The rule system must handle arbitrary data structures without schema pre-registration, support complex boolean logic, provide predictable firewall-style evaluation semantics, and handle missing fields gracefully during schema evolution.

The architecture spans multiple interconnected concerns: expression syntax and evaluation semantics, runtime field resolution with wildcard support, type handling with explicit coercion rules, missing field behavior during schema changes, and operational lifecycle controls for safe production deployment. This hub integrates these interconnected concerns to provide a cohesive view for both implementors and rule authors.

This hub consolidates the complete rule engine architecture, providing strategic overview of the DNF-based expression system, field resolution mechanisms, type coercion strategies, schema evolution handling, and operational controls. It serves as the primary entry point for understanding how TrapperKeeper evaluates rules against streaming data.

## Decision

We implement a **DNF (Disjunctive Normal Form) rule expression system** with schema-agnostic field resolution, explicit type coercion, configurable missing field handling, and operational lifecycle controls.

This document serves as the hub providing strategic architecture overview with cross-references to detailed implementation specifications. It consolidates the rule engine's core design decisions while delegating implementation details to focused spoke documents.

### Validation Domain Boundaries

TrapperKeeper maintains two completely separate validation domains with distinct semantics:

**DOMAIN 1: Internal System Schemas (STRICT)** — All internal schemas (tk-types, database backend, JSONL storage, rule definitions) are strictly validated at API layer with well-defined schemas. Schema evolution refers to changes in THESE internal schemas. Validation failures here are errors that halt processing, not gracefully handled. API requests with invalid rule JSON return 400 errors with specific validation messages.

**DOMAIN 2: External User Data (WEAK, BEST-EFFORT)** — Incoming events, sensor readings, pandas dataframes, and any user-provided data being evaluated by rules use best-effort semantics. We do NOT enforce strict schemas on incoming data. The `on_missing_field` policy is EXCLUSIVELY a runtime evaluation policy for THIS domain. Type coercion and missing field handling enable graceful degradation when external data doesn't match rule expectations, allowing partial evaluation rather than pipeline failures.

**Critical Distinction**: This separation prevents confusion between API validation (strict, catches malformed rule JSON) and runtime evaluation (lenient, handles schema drift in incoming data). Users implementing strict data quality checks must use `on_missing_field="fail"` explicitly in rule conditions—this is NOT the default behavior for incoming data.

### Expression Language and Schema

Rules use OR-of-ANDs structure (DNF) optimized for pre-compilation and visual UI construction. Each rule contains OR groups (`any[]`), each containing AND conditions (`all[]`). Evaluation matches if ANY group matches, where a group matches if ALL conditions match.

**Key Points:**

- DNF structure maps naturally to visual UI (tabs for OR groups, lists for AND conditions)
- Rules pre-compile to nested predicates enabling short-circuit evaluation
- No runtime parsing or expression trees required
- Supports 11 operators: equality (`eq`, `neq`), comparison (`lt`, `lte`, `gt`, `gte`), string (`prefix`, `suffix`), set membership (`in`), existence (`is_null`, `exists`)
- Per-condition configuration with explicit `field_type`, operator, value, and `on_missing_field` policy

**Cross-References:**

- Expression Language Section 1: Complete DNF schema structure with JSON representation
- Expression Language Section 2: Rule fields including version, rule_id, name, description, action, sample_rate, scope
- Expression Language Section 3: Condition schema with field, field_type, op, value, field_ref, on_missing_field
- Expression Language Section 4: Operator set with validation rules

**Example**: A temperature validation rule with multiple OR groups:

```json
{
  "name": "Temperature out of range",
  "action": "drop",
  "any": [
    {
      "all": [
        {
          "field": ["temperature"],
          "field_type": "numeric",
          "op": "lt",
          "value": -40,
          "on_missing_field": "skip"
        }
      ]
    },
    {
      "all": [
        {
          "field": ["temperature"],
          "field_type": "numeric",
          "op": "gt",
          "value": 150,
          "on_missing_field": "skip"
        }
      ]
    }
  ]
}
```

### Field Path Resolution

Field paths reference nested objects and arrays using array notation (`["customer", "address", "zipcode"]`) transmitted as JSON arrays. Wildcards use ANY semantics (match if any element satisfies condition) with optional short-circuit evaluation.

**Key Points:**

- Array representation is language-agnostic and unambiguous
- Wildcard (`*`) evaluation stops at first match (firewall-style)
- Empty arrays result in no match (least intrusive default)
- Resolved paths recorded in events (`matched_field` with indices replacing wildcards)
- Maximum 2 nested wildcards per path (execution cost increases exponentially as 8^n)
- Cross-field comparisons using `field_ref` prohibited from using wildcards (must resolve to single value)

**Cross-References:**

- Field Path Resolution Section 1: Field path notation and representation
- Field Path Resolution Section 2: Wildcard ANY semantics
- Field Path Resolution Section 3: Empty array behavior
- Field Path Resolution Section 4: Short-circuit evaluation as performance optimization
- Field Path Resolution Section 5: Matched field and value resolution
- Field Path Resolution Section 7: Nested wildcard semantics with validation limits
- Field Path Resolution Section 8: Field reference path constraints

**Example**: Wildcard evaluation with nested path:

```json
// Rule: readings[*].temp > 100
// Data: {"readings": [{"temp": 95}, {"temp": 105}, {"temp": 110}]}
// Evaluates: readings[0].temp (95 > 100 = false), readings[1].temp (105 > 100 = true, STOP)
// Result: matched_field = ["readings", 1, "temp"], matched_value = 105
```

### Type System and Coercion

Each condition specifies `field_type` independently (`numeric`, `text`, `boolean`, `any`) with explicit coercion rules. Type coercion applies during evaluation with distinct handling for null-like values vs coercion failures.

**Key Points:**

- **Numeric type**: Strict mode, coerces strings to numbers (`"25"` → 25), rejects non-numeric strings
- **Text type**: Auto-coercion mode, converts all values to strings (100 → "100")
- **Boolean type**: Strict mode, accepts only boolean values
- **Any type**: Lenient mode, Python/JavaScript-style auto-coercion for equality
- **Null-like values**: Treated as missing field, defers to `on_missing_field` policy
- **Coercion failures**: Treated as condition failed (NOT missing field), evaluation continues
- Per-element coercion in wildcard arrays enables best-effort evaluation

**Null vs Coercion Failure**: A critical architectural distinction exists between null-like values and type coercion failures. Null-like values (JSON `null`, missing fields) defer to `on_missing_field` policy (skip/match/fail). Type coercion failures (e.g., `"abc"` coerced to numeric) cause the condition to fail immediately and NEVER trigger `on_missing_field`. This enables best-effort evaluation where unparseable values don't halt processing, while null indicates intentional absence requiring policy decision. See Type System Section 11-12 and Schema Evolution Section 5 for complete semantics.

**Domain Boundary Comparison**:

| Scenario                               | Validation Domain      | Behavior                              | `on_missing_field` Applies?        |
| -------------------------------------- | ---------------------- | ------------------------------------- | ---------------------------------- |
| Missing field in rule definition JSON  | Internal (Strict)      | API validation error (400 response)   | NO - API rejects request           |
| Missing field in incoming event data   | External (Best-effort) | Runtime evaluation policy             | YES - skip/match/fail mode applies |
| Type coercion failure in incoming data | External (Best-effort) | Condition fails, continue evaluation  | NO - treated as condition failed   |
| Null value in incoming event data      | External (Best-effort) | Defers to policy (treated as missing) | YES - skip/match/fail mode applies |

**Cross-References:**

- Type System Section 2: Field type performance characteristics and multipliers
- Type System Section 3: Operator-to-field-type constraints and validation strategy
- Type System Section 4-8: Type coercion rules for numeric, text, boolean, any
- Type System Section 9: IN operator type coercion
- Type System Section 10: Cross-field comparison type coercion
- Type System Section 11-12: Null value semantics and coercion failure distinction

**Example**: Mixed-type rule demonstrating per-condition types:

```json
{
  "any": [
    {
      "all": [
        {
          "field": ["temperature"],
          "field_type": "numeric",
          "op": "gt",
          "value": 100
        },
        {
          "field": ["sensor_id"],
          "field_type": "text",
          "op": "prefix",
          "value": "TEMP-"
        }
      ]
    }
  ]
}
```

### Schema Evolution

Missing field handling uses explicit per-condition configuration with three modes: `skip` (default, least intrusive), `match` (detect incomplete records), `fail` (strict validation). Integrates with field path resolution and type coercion.

**Key Points:**

- Per-condition `on_missing_field` setting enables fine-grained control
- Default `skip` mode prevents pipeline failures from schema drift
- Null values treated as missing field (unified handling simplifies SDK implementation)
- New fields added: existing rules unaffected unless they reference new field
- Fields removed: behavior depends on `on_missing_field` setting
- Field type changes: handled by type coercion with lenient parsing
- Wildcard arrays: per-element missing field handling with first-match short-circuit

**Cross-References:**

- Schema Evolution Section 1: Missing field configuration modes
- Schema Evolution Section 2: Null value semantics
- Schema Evolution Section 3: Schema evolution patterns (add/remove/change fields)
- Schema Evolution Section 4: Interaction with field path resolution
- Schema Evolution Section 5: Integration with type coercion system

**Example**: Per-condition configuration enabling mixed strategies:

```json
{
  "any": [
    {
      "all": [
        {
          "field": ["customer", "id"],
          "op": "exists",
          "on_missing_field": "fail"
        },
        {
          "field": ["customer", "email"],
          "op": "exists",
          "on_missing_field": "skip"
        }
      ]
    }
  ]
}
```

### Rule Lifecycle

Operational controls enable safe production deployment with dry-run testing, emergency pause, individual enable/disable, and simple concurrency handling. Rules propagate via eventual consistency (30s default sync interval).

**Key Points:**

- **Production testing**: Operators create new rule versions with `action: observe` for testing, then create new versions with real actions when satisfied
- **Emergency pause**: In-memory flag returns empty rule set with special ETAG "PAUSED"
- **Enable/disable**: Boolean column filters rules from API query without deletion
- **Concurrency**: Append-only immutable rules. No `modified_at` tracking. Concurrent edits may create orphan versions.
- **Propagation**: Eventual consistency model with 0-30s delay (average 15s)
- **Version creation**: Every modification creates new rule with new `rule_id`, old version soft-deleted

**Cross-References:**

- Lifecycle Section 1: Immutable rules design and version creation flow
- Lifecycle Section 2: Production testing workflow (replacing dry-run)
- Lifecycle Section 3: Emergency pause all rules mechanism
- Lifecycle Section 4: Individual rule enable/disable controls
- Lifecycle Section 5: Change propagation timeline and eventual consistency
- Lifecycle Section 6: Rollback strategy with soft-deleted versions

**Example**: Testing workflow with observe-action versions:

```json
{
  "event_id": "01936a3e-8f2a-7b3c-9d5e-123456789abc",
  "action": "observe",
  "rule": {
    "rule_id": "01936a3e-1234-7b3c-9d5e-abcdef123456",
    "action": "observe"
  }
}
```

### Cost Model Overview

TrapperKeeper uses a deterministic cost model to calculate rule priority, enabling predictable evaluation order and condition-level optimization. The cost model combines two distinct components: field lookup cost (resolving paths in nested structures) and operator evaluation cost (executing comparisons with type-specific and wildcard multipliers).

**Cost Components**:

1. **Lookup Cost**: 128 per string component in field path (wildcards and integer indices are free)
2. **Operator Cost**: Base cost per operator (1 for existence checks, 5 for equality, 7 for comparisons, 10 for string operations)
3. **Field Type Multiplier**: Type complexity adjustment (1× for int/boolean, 4× for float, 48× for string, 128× for any)
4. **Execution Multiplier**: Array expansion cost (`8^n` where n = nested wildcard count)

**Complete Formula**:

```
condition_cost = field_lookup_cost + operator_evaluation_cost
field_lookup_cost = 128 × string_component_count
operator_evaluation_cost = operator_base_cost × field_type_mult × execution_mult
rule_priority = 1000 + Σ(condition_cost) + or_penalty + sample_penalty
```

**Design Rationale**: Constants chosen based on x86_64 CPU benchmarks with typical data processing workloads. String operations are 48× more expensive than integer operations due to memory access patterns and byte-by-byte comparison. Wildcard execution multiplier assumes average array size of 8 elements based on customer data analysis. The `8^n` exponential growth reflects nested iteration cost (outer loop × inner loop).

**Validation Limits**: Maximum 2 nested wildcards per field path enforced at API layer (returns 400 error for violations). Rules with >1 nested wildcard should enable sampling (`sample_rate < 1.0`) to avoid exponential costs. Cross-field comparisons using `field_ref` cannot use wildcards (must reference single value).

For complete operator cost map (11 operators), field type multiplier rationale, calculation algorithm with pseudocode, concrete examples, and full Python reference implementation, see Performance Model spoke.

### Rule Priority and Performance

Rule priority calculated from estimated evaluation cost and used for condition ordering optimization. Combines field lookup cost (128 per string component) and operator evaluation cost (base cost × field type multiplier × wildcard execution multiplier 8^n).

**Key Points:**

- Priority formula: `Σ(condition_cost for each condition)`
- Higher-cost conditions execute last to minimize wasted computation
- Deterministic tie-breaking uses `created_at` timestamp (oldest first)
- Cost-based predicate ordering within `all` groups (existence → equality → comparison → set → string)
- Sample rate fast paths: skip random number generation for 1.0 and 0.0
- Short-circuit evaluation is optional optimization (not semantic requirement)

**Cross-References:**

- See `/doc/05-performance/cost-model.md` Section 2 (Operator Costs, lines 21-41) and Section 3 (Field Type Multipliers, lines 52-82) for base costs
- Complete algorithm documented in Section 5 (lines 204-255) with reference Python implementation (lines 305-387)
- See `/doc/05-performance/README.md` lines 55-89 for strategic overview with cost model rationale and canonical constants explanation

**Example**: Priority calculation with nested wildcards:

```
// Rule: facilities[*].sensors[*].status prefix "ALARM-"
// Field lookup cost = 128 + 0 + 128 + 0 + 128 = 384 (wildcards cost 0)
// Operator cost = 10 (prefix) × 48 (text multiplier) × 64 (8^2 wildcards) = 30,720
// Total condition cost = 384 + 30,720 = 31,104
// Priority = 1000 + 31,104 + 10 (OR penalty) + 0 (sample) = 32,114
```

## Consequences

**Benefits:**

- **Performance**: Pre-compiled predicates enable <1ms evaluation target with short-circuit optimization
- **Usability**: DNF structure maps naturally to visual builder, non-programmers construct complex rules without JSON
- **Predictability**: Explicit OR/AND semantics eliminate ambiguity, first-match firewall behavior matches user expectations
- **Type Safety**: Per-condition `field_type` catches type errors at evaluation, not data ingestion
- **Schema Agnostic**: Handles arbitrary data structures without pre-registration, graceful degradation during schema evolution
- **Operational Safety**: Dry-run, pause, enable/disable controls enable safe production testing and emergency response
- **Granular Control**: Per-condition `on_missing_field` allows different conditions in same rule to handle missing fields differently

**Trade-offs:**

- **Limited Expressiveness**: DNF restricts boolean logic, cannot express `NOT (A OR B)` without De Morgan transformation
- **No Regex**: String matching limited to prefix/suffix, cannot validate complex patterns like email formats
- **Wildcard Limitations**: ANY semantics only, cannot express "ALL elements must satisfy" without multiple conditions
- **Eventual Consistency**: Rule changes propagate with up to 30s delay, sensors may use stale cached rules
- **No Version History**: Cannot inspect previous rule states or revert changes automatically
- **Complexity**: Four field types increase cognitive load, per-condition configuration adds verbosity

## Related Documents

**Consolidated Spokes** (this hub consolidates):

- Expression Language: Maps to Sections 1-4 (DNF schema, rule fields, condition schema, operators)
- Field Path Resolution: Maps to Sections 2-3 (field resolution mechanisms, wildcard semantics)
- Type System and Coercion: Maps to Section 3 (type handling and coercion rules)
- Schema Evolution: Maps to Section 4 (missing field handling)
- Lifecycle: Maps to Section 5 (operational controls)

**Dependencies** (foundational documents):

- Architectural Principles: Implements Schema-Agnostic Architecture through runtime field resolution
- Performance Model and Optimization Strategy: Canonical cost calculation for rule priority (Section 6)

**References** (related hubs/documents):

- Unified Validation and Input Sanitization: Consolidates validation strategy for rule expressions, field paths, type coercion, and missing fields
- Event Schema and Storage: Events capture matched_field, matched_value, and matched_condition for audit trails

**Extended by**:

- Sampling and Performance Optimization: Extends rules with sample_rate field and cost-based predicate ordering
- Batch Processing and Vectorization: Defines vectorized rule evaluation for Pandas/Spark frameworks
