# ADR-016: Type System and Coercion Rules

## Revision log

| Date | Description |
|------|-------------|
| 2025-10-28 | Document created |

## Context

TrapperKeeper evaluates rules against arbitrary data structures with no pre-defined schema. Records may contain fields with inconsistent types across batches (e.g., `age: 25` in one record, `age: "25"` in another). Rules must handle this type variability without failing unexpectedly.

Key challenges:
- **Schema-agnostic processing**: No schema registry to enforce types
- **Mixed-type data sources**: Industrial IoT sensors, streaming JSON, CSV conversions produce inconsistent types
- **User expectations**: Non-programmers expect "25" to match numeric 25 in some contexts
- **Performance**: Type coercion happens in hot path (<1ms target per record)
- **Safety**: Strict typing prevents false matches when semantics differ

Example: A rule checking `temperature > 100` must decide how to handle:
- Numeric: `temperature: 150` → straightforward comparison
- String: `temperature: "150"` → coerce to number or error?
- Boolean: `temperature: true` → reject or attempt conversion?
- Null: `temperature: null` → missing field or error?

## Decision

We will implement a **per-condition type system** with four field types (`numeric`, `text`, `boolean`, `any`), each with explicit coercion rules optimized for performance and safety.

### 1. Field Type System

Each condition specifies `field_type` independently (not rule-level). This enables mixed-type operations where different conditions within the same rule can apply different type semantics.

### 2. Type Coercion Matrix

| field_type | Mode | String → Target | Number → Target | Boolean → Target | Null Handling |
|------------|------|-----------------|-----------------|------------------|---------------|
| `numeric` | Strict | `"25"` → `25` ✓<br>`"abc"` → error | Identity | Error | Missing field |
| `text` | Auto-coerce | Identity | `100` → `"100"` | `true` → `"true"` | Missing field |
| `boolean` | Strict | Error | Error | Identity | Missing field |
| `any` | Lenient | Coerce if comparable | Coerce if comparable | No bool↔string | Missing field |

### 3. Field Type: `numeric`

**Strict mode** - value must be coercible to number.

**Coercion rules**:
- Numeric values: Pass through unchanged (`25 → 25`, `3.14 → 3.14`)
- String values: Parse to number if possible (`"25" → 25`, `"3.14" → 3.14`)
- String values (non-numeric): **Error** (`"abc"`, `"true"`, `""`)
- Boolean values: **Error** (`true`, `false`)
- Null values: Treated as **missing field** (defers to `on_missing_field` setting)

**Usage**: Required for comparison operators (`gt`, `gte`, `lt`, `lte`).

**Example**: See Appendix A for detailed JSON examples of numeric type coercion behavior.

### 4. Field Type: `text`

**Auto-coercion mode** - all values converted to string.

**Coercion rules**:
- String values: Pass through unchanged (`"hello" → "hello"`)
- Numeric values: Convert to string (`100 → "100"`, `3.14 → "3.14"`)
- Boolean values: Convert to string (`true → "true"`, `false → "false"`)
- Null values: Treated as **missing field** (defers to `on_missing_field` setting)

**Usage**: Required for string operators (`prefix`, `suffix`). Optional for equality (`eq`, `neq`).

**Example**: See Appendix A for detailed JSON examples of text type coercion behavior.

### 5. Field Type: `boolean`

**Strict mode** - value must be boolean.

**Coercion rules**:
- Boolean values: Pass through unchanged (`true → true`, `false → false`)
- String values: **Error** (`"true"`, `"false"`, `"1"`)
- Numeric values: **Error** (`1`, `0`)
- Null values: Treated as **missing field** (defers to `on_missing_field` setting)

**Usage**: For equality operators when user wants boolean semantics.

**Example**: See Appendix A for detailed JSON examples of boolean type coercion behavior.

### 6. Field Type: `any`

**Lenient mode** - Python/JavaScript-style auto-coercion for equality checks.

**Coercion rules**:
- Same-type comparisons: Direct comparison (`25 == 25`, `"abc" == "abc"`)
- Cross-type numeric/string: Coerce and compare (`"25" == 25` → true)
- Boolean ↔ String: **Error** (no coercion between booleans and strings)
- Boolean ↔ Numeric: **Error** (no coercion between booleans and numbers)
- Null values: Treated as **missing field** (defers to `on_missing_field` setting)

**Usage**: For equality operators (`eq`, `neq`) when user wants flexible matching.

**Example**: See Appendix A for detailed JSON examples of any type coercion behavior.

### 7. Null Value Semantics

**Null is always treated as missing field**, regardless of `field_type`:

- `temperature: null` with any `field_type` → Defers to `on_missing_field` setting
- This distinguishes between "field exists with null value" and "field doesn't exist"
- In JSON, both cases result in same behavior (least intrusive default)

**Rationale**: Null semantics vary across languages (Python None, JavaScript null, SQL NULL). Treating null as missing field provides consistent, predictable behavior.

### 8. Type Coercion Errors

When type coercion fails, the system treats it as an **error wrapped in `on_missing_field` logic**:

| on_missing_field | Behavior on Coercion Failure |
|------------------|------------------------------|
| `"skip"` (default) | Log warning, skip rule, continue to next rule |
| `"match"` | Treat coercion failure as matching condition |
| `"error"` | Raise exception, fail pipeline |

**Example**: Rule `age > 18` with `field_type="numeric"` evaluates `age: "abc"`
- Coercion attempt: `"abc" → number` fails
- `on_missing_field="skip"`: Log warning, skip rule, continue
- `on_missing_field="error"`: Raise exception, halt pipeline

**Rationale**: Coercion failures indicate data quality issues. Allowing users to configure failure policy aligns with TrapperKeeper's observability goals.

### 9. Wildcard Array Semantics

Type coercion applies **per-element** when evaluating wildcards. Short-circuit evaluation prevents a single bad value from failing the entire array check.

**Evaluation behavior**:
- Iterate through array elements sequentially
- Apply type coercion to each element independently
- Treat impossible coercion as condition failed (not missing field), continue to next element
- Treat `null` elements as missing field (defer to `on_missing_field`)
- Stop on first matching element (short-circuit)

**Example**: See Appendix A for a complete wildcard array coercion example with mixed-type array data.

### 10. Operator-Specific Type Requirements

Certain operators impose type constraints:

| Operator | Required field_type | Rationale |
|----------|---------------------|-----------|
| `gt`, `gte`, `lt`, `lte` | `numeric` | Mathematical comparisons require numeric values |
| `prefix`, `suffix` | `text` | String operations require string values |
| `eq`, `neq` | Any | User chooses type semantics via `field_type` dropdown |
| `is_null`, `exists` | N/A | Existence checks ignore `field_type` |

**UI Enforcement**:
- Comparison operators: Auto-select `field_type="numeric"` (disabled dropdown)
- String operators: Auto-select `field_type="text"` (disabled dropdown)
- Equality operators: User selects from dropdown (`numeric`, `text`, `boolean`, `any`) with help text

### 11. Performance Optimizations

**Fast-path type checks**:
- Use native type checks before coercion (e.g., type inspection in the target language)
- Avoid string parsing for numeric checks when possible
- Cache parsed numeric values for repeated comparisons

**Sample rate optimization**:
- Skip type coercion entirely when `sample_rate = 0.0` (rule disabled)
- When `sample_rate = 1.0`: Normal coercion path (no RNG overhead)

**Future optimizations** (out of scope for MVP):
- JIT compilation of type coercion logic
- Pre-computed type coercion tables for fixed schemas
- Zero-allocation coercion for common cases

## Consequences

### Benefits

1. **Predictable Behavior**: Explicit coercion rules eliminate ambiguity
2. **User-Friendly**: Auto-coercion for text matches user expectations (e.g., `"100"` matches numeric 100 prefix search)
3. **Safety**: Strict modes prevent false matches (e.g., boolean true won't match numeric 1)
4. **Flexible**: Per-condition types enable mixed-type rules
5. **Debuggable**: Type errors logged with clear context (field path, actual value, expected type)
6. **Performance**: Fast-path checks avoid unnecessary coercion overhead

### Tradeoffs

1. **Complexity**: Four field types increase cognitive load for users
2. **Error Handling**: Coercion failures require `on_missing_field` understanding
3. **Inconsistent Defaults**: Different operators have different auto-selected types
4. **Limited Coercion**: No boolean↔string coercion may surprise JavaScript developers
5. **Wildcard Edge Cases**: Per-element coercion in arrays requires careful documentation
6. **No Schema Inference**: Users must explicitly specify types (no automatic detection)

### Operational Implications

1. **User Training**: Requires documentation and examples for each field type
2. **Debugging Support**: Logs must include type coercion failures with full context
3. **Testing**: Each field type requires separate test coverage for coercion paths
4. **UI Design**: Dropdown + help text required for equality operators
5. **Migration Path**: Future schema registry could validate types before evaluation

## Implementation

1. **Define type coercion interface** with FieldType enum (Numeric, Text, Boolean, Any) and CoerceValue function
2. **Implement coercion functions** for each type (see Appendix B for pseudocode)
3. **Integrate with condition evaluation** by checking `field_type` before value comparison
4. **Implement wildcard array handling** with per-element coercion and short-circuit logic
5. **Add comprehensive logging** for coercion failures with full field context
6. **UI Implementation** with auto-selected types for operators and help text for dropdowns
7. **Test Coverage** including unit tests, integration tests, edge cases, and wildcard arrays

For detailed implementation pseudocode, see Appendix B.

## Appendix A: Type Coercion Examples

### Numeric Field Type Example

```json
{
  "field": ["age"],
  "field_type": "numeric",
  "op": "gt",
  "value": 18
}
```

Matches:
- `age: 25` → `25 > 18` ✓
- `age: "25"` → `25 > 18` ✓ (coerced)

Errors:
- `age: "abc"` → Coercion fails → Error (wrapped in `on_missing_field` logic)
- `age: true` → Coercion fails → Error

Skips (when `on_missing_field="skip"`):
- `age: null` → Missing field → Skip rule
- Missing `age` key → Missing field → Skip rule

### Text Field Type Example

```json
{
  "field": ["sensor_id"],
  "field_type": "text",
  "op": "prefix",
  "value": "100"
}
```

Matches:
- `sensor_id: "1003873479"` → `"1003873479".startswith("100")` ✓
- `sensor_id: 1003873479` → `"1003873479".startswith("100")` ✓ (coerced)
- `sensor_id: true` → `"true".startswith("100")` ✗

Skips (when `on_missing_field="skip"`):
- `sensor_id: null` → Missing field → Skip rule

### Boolean Field Type Example

```json
{
  "field": ["is_active"],
  "field_type": "boolean",
  "op": "eq",
  "value": true
}
```

Matches:
- `is_active: true` → `true == true` ✓

Errors:
- `is_active: "true"` → Coercion fails → Error
- `is_active: 1` → Coercion fails → Error

Skips (when `on_missing_field="skip"`):
- `is_active: null` → Missing field → Skip rule

### Any Field Type Example

```json
{
  "field": ["quantity"],
  "field_type": "any",
  "op": "eq",
  "value": 25
}
```

Matches:
- `quantity: 25` → `25 == 25` ✓
- `quantity: "25"` → `25 == 25` ✓ (coerced)

Errors:
- `quantity: true` → No bool↔number coercion → Error

Skips (when `on_missing_field="skip"`):
- `quantity: null` → Missing field → Skip rule

### Wildcard Array Example

```json
{
  "field": ["readings", "*", "temp"],
  "field_type": "numeric",
  "op": "gt",
  "value": 15
}
```

Evaluated against: `readings = [{"temp": 10}, {"temp": "invalid"}, {"temp": 30}]`

**Evaluation sequence**:
1. `readings[0].temp` (10) → numeric → `10 > 15` → false, continue
2. `readings[1].temp` ("invalid") → coercion fails → treat as condition failed, continue
3. `readings[2].temp` (30) → numeric → `30 > 15` → **true, stop**

**Result**: Rule matches with `matched_value = 30`, `matched_field = ["readings", 2, "temp"]`

**Handling null-like values in arrays**:
- `null` elements: Treat as missing field (defer to `on_missing_field`)
- Impossible coercion: Treat as condition failed (not missing field), continue to next element

### Mixed-Type Rule Example

Demonstrates per-condition type system with different field types in same rule:

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

## Appendix B: Type Coercion Implementation Guide

### Interface Definition

```
Define type coercion system:
  - FieldType enum: Numeric, Text, Boolean, Any
  - CoerceValue(value, targetType) function returning (coercedValue, error)
  - Type-specific coercion handlers for each target type
```

### Coercion Function Pseudocode

```
function CoerceNumeric(value):
  if value is numeric:
    return (value, nil)
  if value is string:
    parsed = parse_number(value)
    if parsed is valid:
      return (parsed, nil)
    else:
      return (nil, CoercionError("Cannot convert string to number"))
  return (nil, CoercionError("Type not coercible to numeric"))

function CoerceText(value):
  if value is string:
    return (value, nil)
  if value is numeric or boolean:
    return (to_string(value), nil)
  return (nil, CoercionError("Type not coercible to text"))

function CoerceBoolean(value):
  if value is boolean:
    return (value, nil)
  return (nil, CoercionError("Type not coercible to boolean"))

function CoerceAny(value, compareValue):
  if same_type(value, compareValue):
    return (value, nil)
  if (value is numeric and compareValue is string) or vice versa:
    try coerce_to_common_type(value, compareValue)
    return (coerced, nil or error)
  return (nil, CoercionError("Types not compatible for any coercion"))
```

### Integration with Condition Evaluation

```
function EvaluateCondition(condition, record):
  field_value = resolve_field_path(condition.field, record)

  if field_value is null or field_value is missing:
    handle_missing_field(condition.on_missing_field)
    return

  coerced_value, error = CoerceValue(field_value, condition.field_type)

  if error:
    log_coercion_failure(condition.field, field_value, condition.field_type, error)
    handle_missing_field(condition.on_missing_field)
    return

  result = apply_operator(condition.op, coerced_value, condition.value)
  return result
```

## Related Decisions

**Depends on:**
- **ADR-014: Rule Expression Language** - Defines type system for rule condition evaluation

**Related to:**
- **ADR-015: Field Path Resolution** - Type coercion applies to values resolved through field paths
- **ADR-017: Schema Evolution** - Interacts with missing field handling and null value semantics

## Future Considerations

- **Schema Registry Integration**: Pre-validate field types before evaluation to catch errors earlier
- **User-Defined Type Coercion**: Allow custom coercion functions for specific field paths
- **Type Inference**: Automatically detect types from sample data and suggest field_type
- **Performance Profiling**: Identify hot paths in coercion logic and optimize further
- **Strict Mode Flag**: Add sensor-level flag to disable all coercion and fail on type mismatches
- **Type Statistics**: Collect metrics on coercion success/failure rates per rule for debugging
