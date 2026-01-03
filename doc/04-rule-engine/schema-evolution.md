---
doc_type: spoke
status: active
date_created: 2025-11-07
primary_category: architecture
hub_document: doc/04-rule-engine/README.md
tags:
  - schema-evolution
  - missing-fields
  - on-missing-field
  - on-coercion-fail
  - type-coercion
  - graceful-degradation
cross_cutting:
  - validation
---

# Schema Evolution and Missing Field Handling (Incoming Data)

## Context

TrapperKeeper operates on arbitrary incoming data schemas that evolve over time without pre-registration. Data pipelines frequently encounter schema changes in EXTERNAL USER DATA: new fields added, fields removed, field types changed, and null values appearing in sensor readings, event streams, and pandas dataframes. Traditional schema registries solve this with centralized coordination, but TrapperKeeper's schema-agnostic design requires explicit strategies for handling schema evolution without pipeline failures.

> **CRITICAL DOMAIN CLARIFICATION**: This document covers runtime evaluation semantics for INCOMING USER DATA (Domain 2: weak, best-effort). It does NOT cover TrapperKeeper's internal schemas (tk-types, rule definitions, storage schemas), which use strict API validation (Domain 1). Schema evolution here refers to changes in THE DATA BEING EVALUATED, not our system schemas. For rule definition validation, see Unified Validation and Input Sanitization.

**Hub Document**: This document is part of the Rule Engine Architecture. See [Rule Engine Architecture](README.md) for strategic overview, validation domain boundaries, and relationships to expression language, field resolution, and type coercion.

## Missing Field Configuration

Every condition has an `on_missing_field` configuration option:

| Mode    | Behavior                                                     | Use Case                  |
| ------- | ------------------------------------------------------------ | ------------------------- |
| `skip`  | Field missing → condition doesn't match, continue evaluation | Default, least intrusive  |
| `match` | Field missing → condition matches                            | Detect incomplete records |
| `error` | Field missing → raise exception, fail pipeline               | Strict validation         |

**Default**: `skip` (least intrusive by default)

**Database schema**: `on_missing_field TEXT NOT NULL DEFAULT 'skip'` on condition, not rule

**Rationale**: Per-condition configuration is the simplest data model, matching implementation concerns. Most rules have only one condition, making this the natural granularity. Aligns with Simplicity principle.

## Coercion Failure Configuration

Every condition has an `on_coercion_fail` configuration option that controls behavior when type coercion fails (e.g., attempting to coerce "abc" to numeric):

| Mode    | Behavior                                                      | Use Case                     |
| ------- | ------------------------------------------------------------- | ---------------------------- |
| `skip`  | Coercion fails → condition doesn't match, continue evaluation | Default, best-effort parsing |
| `match` | Coercion fails → condition matches                            | Detect bad data              |
| `error` | Coercion fails → raise exception, fail pipeline               | Strict type validation       |

**Default**: `skip` (best-effort evaluation by default)

**Database schema**: `on_coercion_fail TEXT NOT NULL DEFAULT 'skip'` on condition, not rule

**Rationale**: Parallel to `on_missing_field`, coercion failures represent a distinct failure mode requiring explicit control. Default "skip" enables lenient evaluation where bad values don't halt processing, while "error" mode enables strict type contract enforcement. The "match" mode enables data quality monitoring to detect records with invalid types.

**Critical distinction from `on_missing_field`**:

- `on_missing_field`: Controls behavior when field is missing or null-like
- `on_coercion_fail`: Controls behavior when field is present but type coercion fails
- These are independent policies that can be configured separately per condition

## Null Value Semantics

**Null values treated as missing field**:

- JSON: `{"customer": {"age": null}}`
- Behavior: Same as if `age` key didn't exist
- Defers to `on_missing_field` policy

**Rationale**: Distinguishing "field present but null" from "field absent" adds implementation complexity without clear benefit. Most languages (Python, JavaScript) treat null/undefined similarly in conditionals. Unified handling simplifies SDK implementation.

## Schema Evolution Patterns

### New Fields Added

**Behavior**: Existing rules unaffected unless they explicitly reference the new field

**Example**:

- Original schema: `{"name": "Alice", "age": 30}`
- New schema: `{"name": "Alice", "age": 30, "email": "alice@example.com"}`
- Rule checking `age > 18`: Continues matching as before
- Rule checking `email` field: Matches new records, behavior on old records depends on `on_missing_field`

**Rationale**: Additive changes are safe. Rules only evaluate fields they explicitly reference.

### Fields Removed

**Behavior**: Depends on `on_missing_field` setting

**Example**:

- Original schema: `{"name": "Alice", "age": 30, "department": "Engineering"}`
- New schema: `{"name": "Alice", "age": 30}` (department removed)
- Rule checking `department == "Engineering"`:
  - `on_missing_field="skip"`: Rule doesn't match (default)
  - `on_missing_field="match"`: Rule matches (detects removal)
  - `on_missing_field="error"`: Pipeline fails

**Rationale**: Breaking changes require explicit choice. Default "skip" prevents pipeline failures.

### Field Type Changes

**Behavior**: Handled by type coercion system

**Example**:

- Original schema: `{"user_id": 12345}` (numeric)
- New schema: `{"user_id": "12345"}` (string)
- Rule with `field_type="numeric"`: Coerces `"12345"` → `12345`
- Rule with `field_type="text"`: Coerces `12345` → `"12345"`

**Coercion failure behavior**:

- If coercion impossible (e.g., `"abc"` to numeric): Behavior depends on `on_coercion_fail` policy
  - `on_coercion_fail="skip"` (default): Condition doesn't match, continue evaluation
  - `on_coercion_fail="match"`: Condition matches (detect bad data)
  - `on_coercion_fail="error"`: Raise exception, fail pipeline
- Does NOT trigger `on_missing_field` policy (coercion failures are distinct from missing fields)
- Only null-like values (e.g., `{"user_id": null}`) trigger `on_missing_field` policy

**Rationale**: Lenient type coercion handles gradual migrations. Default "skip" behavior enables best-effort evaluation where bad values don't halt processing. Null-like values are distinct from coercion failures and properly trigger the configured missing field policy. The `on_coercion_fail` policy provides explicit control over type validation strictness.

## Interaction with Field Path Resolution

**Per-condition handling**: Each condition's `on_missing_field` setting controls its behavior independently. A rule with multiple conditions can have different null-handling strategies per condition.

**Example**:

```json
{
  "any": [
    {
      "all": [
        {
          "field": ["customer", "id"],
          "op": "exists",
          "on_missing_field": "error" // Required field
        },
        {
          "field": ["customer", "email"],
          "op": "exists",
          "on_missing_field": "skip" // Optional field
        }
      ]
    }
  ]
}
```

**Nested paths**: Missing field handling applies at any depth

**Scenarios**:

- `customer` field missing → `on_missing_field` policy applies
- `address` field missing from `customer` → `on_missing_field` policy applies
- `zipcode` field missing from `address` → `on_missing_field` policy applies

**Wildcard paths**: Missing field handling per array element

**Behavior**:

- `readings[0].temp` missing → Skip element, try `readings[1]`
- `readings[1].temp = 105` → Match, return this value
- If all elements missing `temp` → `on_missing_field` policy applies to entire condition

**Rationale**: Per-element handling enables resilient evaluation over heterogeneous arrays. First-match short-circuit stops at first valid match.

## Interaction with Type System

> **WARNING - Critical Architectural Boundary**: `on_missing_field` and `on_coercion_fail` are independent policies. Only null-like values (JSON `null`, missing fields) trigger `on_missing_field` policy. Type coercion failures trigger `on_coercion_fail` policy (NOT `on_missing_field`). For strict validation of both field presence and type correctness, set BOTH policies to "error". Setting only `on_missing_field="error"` will NOT catch coercion failures like "abc" -> numeric.

Missing field handling integrates with type coercion. **Critical distinction**: Null-like values and coercion failures are handled by separate policies.

**Processing order**:

1. Resolve field path → Field value or "missing"
2. If missing OR null-like value (e.g., `null`, `None`) → Apply `on_missing_field` policy
3. If present and not null → Attempt type coercion based on `field_type`
4. If coercion fails → Apply `on_coercion_fail` policy
5. If coercion succeeds → Evaluate operator

**Key distinction**:

- **Null-like values**: Trigger `on_missing_field` policy
- **Coercion failures**: Trigger `on_coercion_fail` policy (NOT `on_missing_field`)

**Rationale**: This distinction enables explicit control over two failure modes. Null indicates intentional absence (defer to `on_missing_field` policy), while coercion failure indicates type mismatch (defer to `on_coercion_fail` policy). Separating these policies allows independent configuration of schema evolution tolerance vs. type validation strictness.

### Processing Examples

**Processing flow for `field_type="numeric"` condition**:

```json
{
  "field": ["temperature"],
  "field_type": "numeric",
  "op": "gt",
  "value": 100
}
```

| Data                     | Step 1: Resolve           | Step 2: Missing Check      | Step 3: Coercion             | Step 4: Coercion Fail Check | Step 5: Evaluate   | Final Result                         |
| ------------------------ | ------------------------- | -------------------------- | ---------------------------- | --------------------------- | ------------------ | ------------------------------------ |
| `{}`                     | Field missing             | `on_missing_field` applies | —                            | —                           | —                  | Depends on `on_missing_field` policy |
| `{"temperature": null}`  | null → treated as missing | `on_missing_field` applies | —                            | —                           | —                  | Depends on `on_missing_field` policy |
| `{"temperature": 105}`   | Field present: 105        | Pass                       | 105 (already numeric)        | —                           | `105 > 100` → true | **Match**                            |
| `{"temperature": "105"}` | Field present: "105"      | Pass                       | "105" → 105 (coerce success) | —                           | `105 > 100` → true | **Match**                            |
| `{"temperature": "abc"}` | Field present: "abc"      | Pass                       | "abc" → fail                 | `on_coercion_fail` applies  | —                  | Depends on `on_coercion_fail` policy |

**Configuration mode implications**:

**IMPORTANT**: `on_missing_field` and `on_coercion_fail` are independent policies controlling distinct failure modes. Configure both to achieve desired validation strictness.

**`on_missing_field` modes**:

**Mode: `skip` (default)**

- Missing field or null-like value: Rule doesn't match, continue to next rule
- Use case: Production pipelines that should tolerate schema drift

**Mode: `match`**

- Missing field or null-like value: Rule matches
- Use case: Data quality monitoring to detect incomplete records

**Mode: `error`**

- Missing field or null-like value: Raise exception, fail pipeline
- Use case: Strict validation where schema contract must be enforced for field presence

**`on_coercion_fail` modes**:

**Mode: `skip` (default)**

- Coercion failure: Condition doesn't match, continue evaluation
- Use case: Best-effort parsing where type mismatches should be tolerated

**Mode: `match`**

- Coercion failure: Condition matches
- Use case: Data quality monitoring to detect records with invalid types

**Mode: `error`**

- Coercion failure: Raise exception, fail pipeline
- Use case: Strict validation where type contract must be enforced

**Combined strict validation**:

Setting both `on_missing_field="error"` AND `on_coercion_fail="error"` enforces strict validation that catches:

- Missing fields and null values (via `on_missing_field="error"`)
- Type coercion failures (via `on_coercion_fail="error"`)

This combination ensures complete validation of field presence AND type correctness.

## Configuration Examples

### Example 1: Detect missing age field (match mode)

```json
{
  "rule_id": "01936a3e-1234-7b3c-9d5e-abcdef123456",
  "name": "Detect missing age field",
  "action": "observe",
  "any": [
    {
      "all": [
        {
          "field": ["customer", "age"],
          "field_type": "numeric",
          "op": "exists",
          "value": null,
          "on_missing_field": "match"
        }
      ]
    }
  ]
}
```

### Example 2: Skip records without email (skip mode - default)

```json
{
  "rule_id": "01936a3e-5678-7b3c-9d5e-abcdef789012",
  "name": "Validate email format",
  "action": "observe",
  "any": [
    {
      "all": [
        {
          "field": ["contact", "email"],
          "field_type": "text",
          "op": "prefix",
          "value": "@",
          "on_missing_field": "skip"
        }
      ]
    }
  ]
}
```

### Example 3: Enforce required fields (error mode)

```json
{
  "rule_id": "01936a3e-9abc-7b3c-9d5e-abcdef345678",
  "name": "Require user_id field",
  "action": "observe",
  "any": [
    {
      "all": [
        {
          "field": ["user_id"],
          "field_type": "text",
          "op": "exists",
          "value": null,
          "on_missing_field": "error"
        }
      ]
    }
  ]
}
```

### Example 4: Mixed null-handling strategies within single rule

```json
{
  "rule_id": "01936a3e-def0-7b3c-9d5e-abcdef456789",
  "name": "Validate customer with required and optional fields",
  "action": "observe",
  "any": [
    {
      "all": [
        {
          "field": ["customer", "id"],
          "field_type": "text",
          "op": "exists",
          "value": null,
          "on_missing_field": "error" // Required field
        },
        {
          "field": ["customer", "email"],
          "field_type": "text",
          "op": "exists",
          "value": null,
          "on_missing_field": "skip" // Optional field
        }
      ]
    }
  ]
}
```

### Example 5: Strict type validation with on_coercion_fail

```json
{
  "rule_id": "01936a3e-1111-7b3c-9d5e-abcdef111111",
  "name": "Validate numeric temperature with strict typing",
  "action": "observe",
  "any": [
    {
      "all": [
        {
          "field": ["temperature"],
          "field_type": "numeric",
          "op": "gt",
          "value": 100,
          "on_missing_field": "error",
          "on_coercion_fail": "error"
        }
      ]
    }
  ]
}
```

**Behavior**:

- `{"temperature": 105}`: Match (numeric, > 100)
- `{"temperature": "105"}`: Match (coerces to 105, > 100)
- `{"temperature": "abc"}`: Raise exception (coercion fails, on_coercion_fail="error")
- `{}`: Raise exception (missing field, on_missing_field="error")
- `{"temperature": null}`: Raise exception (null treated as missing, on_missing_field="error")

### Example 6: Detect bad data with on_coercion_fail match mode

```json
{
  "rule_id": "01936a3e-2222-7b3c-9d5e-abcdef222222",
  "name": "Detect non-numeric temperature values",
  "action": "observe",
  "any": [
    {
      "all": [
        {
          "field": ["temperature"],
          "field_type": "numeric",
          "op": "gt",
          "value": 0,
          "on_coercion_fail": "match"
        }
      ]
    }
  ]
}
```

**Behavior**:

- `{"temperature": 105}`: No match (coercion succeeds, but used for comparison)
- `{"temperature": "abc"}`: **Match** (coercion fails, on_coercion_fail="match" triggers)
- `{}`: No match (missing field, default on_missing_field="skip")

### Example 7: Combined policies for comprehensive validation

```json
{
  "rule_id": "01936a3e-3333-7b3c-9d5e-abcdef333333",
  "name": "Strict validation with separate missing and coercion policies",
  "action": "observe",
  "any": [
    {
      "all": [
        {
          "field": ["user", "age"],
          "field_type": "numeric",
          "op": "gte",
          "value": 18,
          "on_missing_field": "skip",
          "on_coercion_fail": "error"
        }
      ]
    }
  ]
}
```

**Behavior**:

- `{"user": {"age": 25}}`: Match (numeric, >= 18)
- `{"user": {"age": "25"}}`: Match (coerces to 25, >= 18)
- `{"user": {"age": "abc"}}`: Raise exception (coercion fails, on_coercion_fail="error")
- `{}`: No match (missing field, on_missing_field="skip")
- `{"user": {}}`: No match (missing age field, on_missing_field="skip")

This combination tolerates schema evolution (missing fields) while enforcing strict type validation (coercion failures).

## Edge Cases and Limitations

**Known Limitations**:

- **No Schema Validation**: System cannot detect schema errors before evaluation
- **Duplicate Behavior**: Type coercion failures and missing fields handled similarly (intentional simplicity)
- **No "Field Present" Check**: Cannot distinguish null from missing without explicit `exists`/`is_null` operators
- **Migration Visibility**: Silent schema changes with `skip` mode may hide data quality issues

**Edge Cases**:

- **Deeply nested missing fields**: `on_missing_field` applies at any nesting level
- **Empty objects**: `{}` evaluated as missing field if path expects nested structure
- **Array index out of bounds**: Treated as missing field

## Related Documents

**Dependencies** (read these first):

- Field Path Resolution: Provides runtime field path resolution mechanism extended with missing field handling
- Type System and Coercion: Defines type coercion rules that interact with missing field semantics

**Related Spokes** (siblings in this hub):

- Expression Language: Defines `on_missing_field` configuration per condition
- Lifecycle: Operational controls for testing schema evolution with dry-run mode

**Extended by** (documents building on this):

- Unified Validation and Input Sanitization: Missing field validation (on_missing_field policy enforcement) consolidated
