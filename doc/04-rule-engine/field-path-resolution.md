---
doc_type: spoke
status: active
date_created: 2025-11-07
primary_category: architecture
hub_document: doc/04-rule-engine/README.md
tags:
  - field-paths
  - wildcards
  - any-semantics
  - schema-agnostic
cross_cutting:
  - validation
---

# Field Path Resolution and Wildcard Semantics

## Context

TrapperKeeper operates on arbitrary data structures without schema pre-registration. Rules must reference fields in nested objects and arrays using a consistent, predictable notation that works identically across Python (Pandas), Java (Spark), and Go frameworks while completing path resolution in microseconds.

**Hub Document**: This document is part of the Rule Engine Architecture. See [Rule Engine Architecture](README.md) for strategic overview and relationships to expression language, type coercion, and schema evolution.

## Field Path Notation

Rules reference fields using dot/bracket syntax, transmitted as JSON arrays:

| User Input Syntax          | JSON Path Array                      | Meaning                                 |
| -------------------------- | ------------------------------------ | --------------------------------------- |
| `customer.address.zipcode` | `["customer", "address", "zipcode"]` | Nested objects                          |
| `sensors[3].value`         | `["sensors", 3, "value"]`            | Array index (zero-based)                |
| `readings[*].temp`         | `["readings", "*", "temp"]`          | Wildcard (ANY semantics)                |
| `data["field.with.dots"]`  | `["data", "field.with.dots"]`        | Escaped keys for fields containing dots |

**Rationale**:

- **Array representation**: JSON arrays are unambiguous and language-agnostic (no parsing ambiguity)
- **Dot notation in UI**: Familiar syntax for rule authors, converted to array by UI
- **Bracket escaping**: Handles edge case of field names containing dots (e.g., DNS records, metric names)
- **Zero-based indexing**: Aligns with Python, Java, JavaScript conventions

## Wildcard Semantics (ANY, not ALL)

Wildcards use **ANY semantics**: A condition matches if **any element** satisfies it.

**Example**:

```json
{
  "readings": [{ "temp": 10 }, { "temp": 30 }, { "temp": 50 }]
}
```

Rule: `readings[*].temp > 15` → **Matches** (because readings[1].temp = 30 satisfies condition)

**Rationale**:

- **Firewall-style semantics**: "Block if any temperature is too high" is more intuitive than "Block if all temperatures are too high"
- **Error detection use case**: Data quality rules typically flag problematic records, not validate all elements
- **Performance**: Short-circuit evaluation stops at first match (no need to check remaining elements)
- **Aligns with EXISTS semantics**: Similar to SQL `WHERE EXISTS (SELECT ... WHERE condition)`

**Alternative considered and rejected**:

- **ALL semantics**: Would require checking every array element, slower and less useful for error detection
- Example: `readings[*].temp > 15` with ALL semantics would only match if readings[0], readings[1], AND readings[2] all exceed 15

**Note**: Wildcards are only supported in the primary `field` path. The `field_ref` path (for cross-field comparisons) cannot contain wildcards.

## Empty Array Behavior

Empty arrays are treated as **all elements missing**, deferring to the `on_missing_field` policy:

**Example**:

```json
{ "readings": [] }
```

Rule: `readings[*].temp > 15`

- With `on_missing_field="skip"` (default): **Does not match** (least intrusive)
- With `on_missing_field="match"`: **Matches** (detects incomplete data)
- With `on_missing_field="error"`: **Raises exception** (strict validation)

**Rationale**:

- **Semantic consistency**: Empty array = vacuously "all elements missing field"
- **Unified policy control**: Same `on_missing_field` setting controls both missing fields and empty arrays
- **Schema evolution alignment**: Empty arrays follow same evolution semantics as missing fields
- **Default behavior preserved**: `on_missing_field="skip"` (default) maintains least intrusive principle

## First-Match Short-Circuit Evaluation (Performance Optimization)

Wildcard evaluation **may stop at the first matching element** as a performance optimization:

**Example**:

```json
{
  "readings": [{ "temp": 10 }, { "temp": 30 }, { "temp": 50 }]
}
```

Rule: `readings[*].temp > 15` → May evaluate readings[0] (10 > 15 = false), then readings[1] (30 > 15 = **true**), **stops** without checking readings[2]

**CRITICAL**: Short-circuit behavior is an **optimization, not a semantic requirement**:

- **SDK discretion**: Implementors may choose whether to implement short-circuit based on framework constraints
- **Applies per-condition**: Short-circuit operates within a single condition evaluation, not across records in a batch
- **Not per-batch**: Batch processing frameworks evaluate all rows in the batch; short-circuit does not abandon the entire batch after first match
- **Deterministic when used**: If implemented, always returns the earliest matching element for predictability

**Rationale**:

- **Performance option**: Avoids unnecessary checks once match found (firewall-style approach)
- **Framework flexibility**: Vectorized operations (Pandas, Spark) may evaluate all elements for performance reasons
- **Implementation freedom**: SDK authors choose appropriate strategy for their execution model
- **Consistent results**: Whether short-circuited or not, semantic result (match/no-match) must be identical

## Matched Field and Value Resolution

When a wildcard matches, the **resolved index** and **actual value** are recorded:

**Example**:

```json
{
  "readings": [{ "temp": 10 }, { "temp": 30 }, { "temp": 50 }]
}
```

Rule: `readings[*].temp > 15`

- `matched_field` = `["readings", 1, "temp"]` (wildcard resolved to index 1)
- `matched_value` = `30` (actual value that triggered match)

**Rationale**:

- **Auditability**: Events show exactly which array element caused the match
- **Debugging**: Operators can identify problematic records in large arrays
- **Wildcard resolution**: Replace `*` with actual index for precise field path

## Type Coercion Integration

Field resolution interacts with type coercion during wildcard evaluation. **Critical distinction**: Null-like values and coercion failures are handled differently.

**Null-like values**: Treated as **missing field**, deferred to `on_missing_field` setting:

```json
{
  "readings": [{ "temp": null }, { "temp": 30 }]
}
```

Rule: `readings[*].temp > 15` with `on_missing_field="skip"`

- Evaluates readings[0].temp (null → **null-like value** → treated as **missing field** → skip to next element)
- Evaluates readings[1].temp (30 > 15 = **true**, match)

**Impossible coercion**: Treat as **condition failed** (NOT missing field), continue to next element:

```json
{
  "readings": [{ "temp": 10 }, { "temp": "invalid" }, { "temp": 30 }]
}
```

Rule: `readings[*].temp > 15` with `field_type="numeric"`

- Evaluates readings[0].temp (10 > 15 = false)
- Evaluates readings[1].temp ("invalid" cannot coerce to numeric → **coercion fails** → **condition failed**, continue)
- Evaluates readings[2].temp (30 > 15 = **true**, match)
- `matched_field` = `["readings", 2, "temp"]` (skipped index 1)
- `matched_value` = `30`

**Rationale**:

- **Fail-safe wildcard evaluation**: Type coercion failures don't halt entire array scan
- **Continue on coercion failure**: Check remaining elements rather than erroring immediately
- **Distinct null handling**: Null-like values trigger `on_missing_field` policy, but coercion failures do not
- **Best-effort evaluation**: Enables processing arrays with mixed-quality data

## Nested Wildcard Semantics

Nested wildcards (e.g., `departments[*].employees[*].salary`) are **fully supported** with ANY semantics at all nesting levels.

**IMPORTANT**: Complete nested wildcard validation limits, cost analysis, and enforcement mechanisms are defined in the Performance Model and Optimization Strategy hub (Section 8). This section provides semantic behavior only.

**Semantics**: ANY requirement applies at all nested levels. A nested wildcard matches if **any outer element** contains **any inner element** that satisfies the condition.

### Example 1: Double nesting

```json
{
  "departments": [
    {
      "name": "Engineering",
      "employees": [
        { "name": "Alice", "salary": 80000 },
        { "name": "Bob", "salary": 120000 }
      ]
    },
    {
      "name": "Sales",
      "employees": [{ "name": "Charlie", "salary": 60000 }]
    }
  ]
}
```

Rule: `departments[*].employees[*].salary > 100000` → **Matches**

- Reason: ANY department (Engineering) has ANY employee (Bob) with salary > 100000
- `matched_field` = `["departments", 0, "employees", 1, "salary"]`
- `matched_value` = `120000`

### Example 2: Triple nesting

```json
{
  "regions": [
    {
      "departments": [
        {
          "teams": [{ "budget": 50000 }]
        }
      ]
    }
  ]
}
```

Rule: `regions[*].departments[*].teams[*].budget > 40000` → **Matches**

- Evaluates: ANY region → ANY department → ANY team → budget > 40000

**Short-circuit evaluation**: Nested wildcard evaluation may stop at the first matching path found, traversing in document order (depth-first, left-to-right). This is an optimization, not a requirement.

**Performance implications**: Nested wildcards have multiplicative performance cost. Execution cost increases exponentially (8^n) with nested wildcards.

**Nested Wildcard Validation**:

**Limit**: Maximum 2 nested wildcards per field path

**Rationale**: Execution cost increases exponentially (8^n) with nested wildcards. See Performance Model and Optimization Strategy hub (Section 8) for complete validation rules and cost analysis.

**Examples**:

- ✅ `items[*].tags[*]` = 2 wildcards (allowed)
- ❌ `a[*].b[*].c[*]` = 3 wildcards (REJECTED)

## Field Reference Paths (field_ref)

Rules support cross-field comparisons using `field_ref`, which references a second field in the same record:

```json
{
  "field": ["temperature"],
  "field_type": "numeric",
  "op": "gt",
  "field_ref": ["calibrated_max"]
}
```

**Field Reference Constraints**:

**No wildcards allowed** in `field_ref` paths:

- `field_ref` must resolve to a single value
- Wildcards (`*`) are prohibited
- Integer array indices are permitted
- Validation enforced at UI, rule creation, and runtime

**Examples**:

```json
// VALID field_ref paths
["threshold"]                    // Simple field
["config", "max_value"]          // Nested field
["sensors", 0, "calibration"]    // Array with integer index

// INVALID field_ref paths
["sensors", "*", "calibration"]  // ❌ Wildcard not allowed
["facilities", "*", "max"]       // ❌ Wildcard not allowed
```

**Rationale**:

- Wildcards would make comparison ambiguous (compare to which array element?)
- Single-value constraint ensures deterministic comparison
- Use cases requiring wildcard comparisons can restructure data or use pre-processing

**Field Resolution**:

- `field_ref` path resolved using same mechanism as `field` path
- Missing field handling: If `field_ref` path not found, applies condition's `on_missing_field` behavior
- Type coercion: Both `field` and `field_ref` values coerced according to `field_type`

**Cost Calculation**:

- `field_ref` lookup cost calculated same as `field` path: 128 per string component
- Total lookup cost = `field` path cost + `field_ref` path cost
- Example: `field=["temp"]` (128) + `field_ref=["threshold"]` (128) = 256

**Validation Strategy**:

1. **UI Prevention**: Disable wildcard input in field_ref selector
2. **Rule Creation**: API validates field_ref contains no wildcards, returns 400 if found
3. **Runtime Enforcement**: SDK validates field_ref has no wildcards when evaluating condition

**Examples**:

```json
// Industrial IoT: Dynamic threshold comparison
{
  "field": ["reading_value"],
  "field_type": "numeric",
  "op": "gt",
  "field_ref": ["calibrated_max"],
  "on_missing_field": "skip"
}

// Record: {"reading_value": 105, "calibrated_max": 100}
// Evaluation: 105 > 100 → Match

// Missing field_ref scenario
// Record: {"reading_value": 105}
// Evaluation: calibrated_max missing → Applies on_missing_field="skip" → No match
```

## Maximum Path Depth

Field path depth is limited to **16 segments maximum**.

**Example**: `["level1", "level2", ..., "level16"]` is valid; 17+ segments rejected at rule creation.

**Rationale**:

- **Defensive limits**: Explicit bounds prevent unexpected performance degradation from pathological inputs
- **SRE philosophy**: TrapperKeeper follows defense-in-depth -- explicit limits everywhere prevent edge cases from causing production incidents
- **Practical sufficiency**: Real-world data structures rarely exceed 10 levels of nesting

## Edge Cases and Limitations

**Known Limitations**:

- **No ALL Semantics**: Cannot express "match if all elements satisfy condition" (workaround: invert logic with neq/lt)
- **No Slicing**: Cannot reference ranges like `readings[0:5].temp` (MVP limitation)
- **Non-Guaranteed Short-Circuit**: Large arrays may be fully evaluated depending on SDK implementation
- **Type Error Skipping**: Coercion failures in arrays silently continue (may miss data quality issues)
- **Implementation Variation**: Different SDKs may have different short-circuit behavior (though results must be semantically identical)
- **Nested Wildcard Performance**: Deeply nested wildcards have exponential performance impact (8^n where n = number of wildcards)
- **No Wildcards in field_ref**: Cannot compare field values against arrays of thresholds using wildcards (requires data restructuring or pre-processing)

**Edge Cases**:

- **Empty arrays**: Always result in no match (consistent with ANY semantics)
- **Null elements in arrays**: Trigger `on_missing_field` policy per element
- **Coercion failures in arrays**: Treated as condition failed, continue to next element

## Related Documents

**Dependencies** (read these first):

- Rule Expression Language: Extends the rule language with detailed field path resolution semantics
- Performance Model and Optimization Strategy: Nested wildcard validation limits

**Related Spokes** (siblings in this hub):

- Type System and Coercion: Field resolution results are processed through type coercion
- Schema Evolution: Handles missing fields during path resolution

**Extended by** (documents building on this):

- Sampling and Performance Optimization: Optimizes field extraction and wildcard evaluation
- Batch Processing and Vectorization: Defines field path semantics for vectorized operations
- Unified Validation and Input Sanitization: Field path resolution validation (wildcard syntax, nested wildcard limits) consolidated
