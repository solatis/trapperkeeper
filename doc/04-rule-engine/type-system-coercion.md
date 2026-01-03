---
doc_type: spoke
status: active
primary_category: architecture
hub_document: doc/04-rule-engine/README.md
tags:
  - type-system
  - type-coercion
  - field-types
  - int-float-string-boolean
cross_cutting:
  - validation
---

# Type System and Coercion Rules

## Context

TrapperKeeper evaluates rules against arbitrary data structures with no pre-defined schema. Records may contain fields with inconsistent types across batches (e.g., `age: 25` in one record, `age: "25"` in another). Rules must handle this type variability without failing unexpectedly while maintaining performance (<1ms target per record).

**Hub Document**: This document is part of the Rule Engine Architecture. See [Rule Engine Architecture](README.md) for strategic overview and relationships to expression language, field resolution, and schema evolution.

## Field Type System

Each condition specifies `field_type` independently (not rule-level). This enables mixed-type operations where different conditions within the same rule can apply different type semantics.

### Supported Field Types

- **`int`**: Strict mode for integer numbers
- **`float`**: Strict mode for floating-point numbers
- **`string`**: Auto-coercion mode, converts all values to strings
- **`boolean`**: Strict mode, accepts only boolean values
- **`any`**: Lenient mode, Python/JavaScript-style auto-coercion

## Field Type Performance Characteristics

Field types have different performance costs based on CPU operation complexity. These field type multipliers are applied to operator costs in priority calculation and are defined canonically in the Performance Model and Optimization Strategy hub (Section 4).

**Note**: Field type multipliers are maintained in the Performance hub as the single source of truth. All updates to field type performance characteristics must be made there, not here.

These multipliers reflect real-world CPU performance differences measured through benchmarks. They guide the rule engine's priority-based evaluation order to execute cheaper operations first.

## Operator-to-Field-Type Constraints

Not all operators support all field types. The following table defines valid combinations:

| Operator  | Supported field_types | Validation                         |
| --------- | --------------------- | ---------------------------------- |
| `exists`  | all types             | No type checking needed            |
| `is_null` | all types             | No type checking needed            |
| `eq`      | all types             | Type coercion per field_type rules |
| `neq`     | all types             | Type coercion per field_type rules |
| `lt`      | `any`, `int`, `float` | Strict numeric requirement         |
| `lte`     | `any`, `int`, `float` | Strict numeric requirement         |
| `gt`      | `any`, `int`, `float` | Strict numeric requirement         |
| `gte`     | `any`, `int`, `float` | Strict numeric requirement         |
| `prefix`  | `any`, `string`       | Auto-coercion to string            |
| `suffix`  | `any`, `string`       | Auto-coercion to string            |
| `in`      | all types             | Type coercion per field_type rules |

**Validation Strategy**:

1. **UI Prevention**: Make invalid operator/field_type combinations impossible to select
   - Example: If user selects `gt` operator, field_type dropdown shows only `any`, `int`, `float`

2. **Rule Creation Validation**: API validates operator/field_type compatibility before persisting
   - Return 400 Bad Request with clear error message for invalid combinations
   - Example: `{"error": "Operator 'prefix' requires field_type 'string' or 'any', got 'int'"}`

3. **Runtime Validation**: Enforce constraints when evaluating rules (defense in depth)
   - Same validation as creation time
   - Protects against direct database modifications or schema evolution issues
   - Log warning in debug mode, error in release mode

## Type Coercion Matrix

| field_type | Mode        | String -> Target                            | Number -> Target     | Boolean -> Target  | Null Handling |
| ---------- | ----------- | ------------------------------------------- | -------------------- | ------------------ | ------------- |
| `int`      | Strict      | `"25"` -> `25` [OK]<br>`"abc"` -> error     | Identity             | Error              | Missing field |
| `float`    | Strict      | `"3.14"` -> `3.14` [OK]<br>`"abc"` -> error | Identity             | Error              | Missing field |
| `string`   | Auto-coerce | Identity                                    | `100` -> `"100"`     | `true` -> `"true"` | Missing field |
| `boolean`  | Strict      | Error                                       | Error                | Identity           | Missing field |
| `any`      | Lenient     | Coerce if comparable                        | Coerce if comparable | No bool<->string   | Missing field |

## Field Type: int

**Strict mode** - value must be coercible to integer.

**Coercion rules**:

- Integer values: Pass through unchanged (`25 -> 25`)
- Float values: Truncate to integer (`3.14 -> 3`)
- String values: Parse to integer if possible (`"25" -> 25`)
- String values (non-numeric): **Coercion fails** -> Treated as condition failed (`"abc"`, `"true"`, `""`)
- Boolean values: **Coercion fails** -> Treated as condition failed (`true`, `false`)
- Null values: Treated as **missing field** (defers to `on_missing_field` setting)

**Usage**: Required for comparison operators (`gt`, `gte`, `lt`, `lte`) when working with integer values.

**Example**:

```json
{
  "field": ["age"],
  "field_type": "int",
  "op": "gt",
  "value": 18
}
```

Matches:

- `age: 25` -> `25 > 18` [OK]
- `age: "25"` -> `25 > 18` [OK] (coerced)

Does not match (condition fails):

- `age: "abc"` -> Coercion fails -> Condition evaluates to false (NOT treated as missing field)
- `age: true` -> Coercion fails -> Condition evaluates to false (NOT treated as missing field)

Applies `on_missing_field` policy (when `on_missing_field="skip"` -> skip rule):

- `age: null` -> Null-like value -> Treated as missing field -> Skip rule
- Missing `age` key -> Missing field -> Skip rule

## Field Type: float

**Strict mode** - value must be coercible to floating-point number.

**Coercion rules**:

- Float values: Pass through unchanged (`3.14 -> 3.14`)
- Integer values: Convert to float (`25 -> 25.0`)
- String values: Parse to float if possible (`"3.14" -> 3.14`, `"25" -> 25.0`)
- String values (non-numeric): **Coercion fails** -> Treated as condition failed (`"abc"`, `"true"`, `""`)
- Boolean values: **Coercion fails** -> Treated as condition failed (`true`, `false`)
- Null values: Treated as **missing field** (defers to `on_missing_field` setting)

**Usage**: Required for comparison operators (`gt`, `gte`, `lt`, `lte`) when working with floating-point values.

**Example**:

```json
{
  "field": ["temperature"],
  "field_type": "float",
  "op": "gt",
  "value": 98.6
}
```

Matches:

- `temperature: 99.5` -> `99.5 > 98.6` [OK]
- `temperature: "99.5"` -> `99.5 > 98.6` [OK] (coerced)
- `temperature: 100` -> `100.0 > 98.6` [OK] (coerced from int)

Does not match (condition fails):

- `temperature: "invalid"` -> Coercion fails -> Condition evaluates to false (NOT treated as missing field)
- `temperature: true` -> Coercion fails -> Condition evaluates to false (NOT treated as missing field)

Applies `on_missing_field` policy (when `on_missing_field="skip"` -> skip rule):

- `temperature: null` -> Null-like value -> Treated as missing field -> Skip rule
- Missing `temperature` key -> Missing field -> Skip rule

## Field Type: string

**Auto-coercion mode** - all values converted to string.

**Coercion rules**:

- String values: Pass through unchanged (`"hello" -> "hello"`)
- Numeric values: Convert to string (`100 -> "100"`, `3.14 -> "3.14"`)
- Boolean values: Convert to string (`true -> "true"`, `false -> "false"`)
- Null values: Treated as **missing field** (defers to `on_missing_field` setting)

**Usage**: Required for string operators (`prefix`, `suffix`). Optional for equality (`eq`, `neq`).

**Example**:

```json
{
  "field": ["sensor_id"],
  "field_type": "string",
  "op": "prefix",
  "value": "100"
}
```

Matches:

- `sensor_id: "1003873479"` -> `"1003873479".startswith("100")` [OK]
- `sensor_id: 1003873479` -> `"1003873479".startswith("100")` [OK] (coerced)
- `sensor_id: true` -> `"true".startswith("100")` [FAIL]

Skips (when `on_missing_field="skip"`):

- `sensor_id: null` -> Missing field -> Skip rule

## Field Type: boolean

**Strict mode** - value must be boolean.

**Coercion rules**:

- Boolean values: Pass through unchanged (`true -> true`, `false -> false`)
- String values: **Coercion fails** -> Treated as condition failed (`"true"`, `"false"`, `"1"`)
- Numeric values: **Coercion fails** -> Treated as condition failed (`1`, `0`)
- Null values: Treated as **missing field** (defers to `on_missing_field` setting)

**Usage**: For equality operators when user wants boolean semantics.

**Example**:

```json
{
  "field": ["is_active"],
  "field_type": "boolean",
  "op": "eq",
  "value": true
}
```

Matches:

- `is_active: true` -> `true == true` [OK]

Does not match (condition fails):

- `is_active: "true"` -> Coercion fails -> Condition evaluates to false (NOT treated as missing field)
- `is_active: 1` -> Coercion fails -> Condition evaluates to false (NOT treated as missing field)

Applies `on_missing_field` policy (when `on_missing_field="skip"` -> skip rule):

- `is_active: null` -> Null-like value -> Treated as missing field -> Skip rule

## Field Type: any

**Lenient mode** - Python/JavaScript-style auto-coercion for equality checks.

**Coercion rules**:

- Same-type comparisons: Direct comparison (`25 == 25`, `"abc" == "abc"`)
- Cross-type numeric/string: Coerce and compare (`"25" == 25` -> true)
- Boolean <-> String: **Coercion fails** -> Treated as condition failed (no coercion between booleans and strings)
- Boolean <-> Numeric: **Coercion fails** -> Treated as condition failed (no coercion between booleans and numbers)
- Null values: Treated as **missing field** (defers to `on_missing_field` setting)

**Usage**: For equality operators (`eq`, `neq`) when user wants flexible matching.

**Example**:

```json
{
  "field": ["quantity"],
  "field_type": "any",
  "op": "eq",
  "value": 25
}
```

Matches:

- `quantity: 25` -> `25 == 25` [OK]
- `quantity: "25"` -> `25 == 25` [OK] (coerced)

Does not match (condition fails):

- `quantity: true` -> No bool<->number coercion -> Condition evaluates to false (NOT treated as missing field)

Applies `on_missing_field` policy (when `on_missing_field="skip"` -> skip rule):

- `quantity: null` -> Null-like value -> Treated as missing field -> Skip rule

## IN Operator Type Coercion

The `in` operator checks if a field value exists in a set of values:

```json
{
  "field": ["status"],
  "field_type": "string",
  "op": "in",
  "values": ["active", "pending", "processing"]
}
```

**Coercion rules**:

- Field value coerced according to `field_type` (same as other operators)
- All values in `values` array must be same type
- Comparison uses coerced field value against array elements
- **Null handling**: Null field value treated as missing field (defers to `on_missing_field`)
- **Type mismatch**: If field value cannot coerce, condition fails (not missing field)

**Examples**:

```json
// field_type="string", value auto-coerces
{"status": 100} with values=["100", "200"]
-> Coerce 100 -> "100" -> Match (in array)

// field_type="int", string value coerces
{"age": "25"} with values=[18, 25, 65]
-> Coerce "25" -> 25 -> Match (in array)

// Coercion failure
{"age": "invalid"} with field_type="int", values=[18, 25, 65]
-> Coercion fails -> Condition fails (NOT missing field)
```

## Cross-Field Comparison Type Coercion

When using `field_ref` for cross-field comparisons, type coercion applies to both field values:

```json
{
  "field": ["temp"],
  "field_type": "float",
  "op": "gt",
  "field_ref": ["threshold"]
}
```

**Coercion process**:

1. Resolve `field` path -> extract value
2. Resolve `field_ref` path -> extract reference value
3. Apply type coercion to both values based on `field_type`
4. Compare coerced values using operator

**Example**:

```json
// Record: {"temp": 105, "threshold": "100"}
// field_type="float"

Step 1: temp = 105 (integer)
Step 2: threshold = "100" (string)
Step 3: Coerce 105 -> 105.0, "100" -> 100.0 (float)
Step 4: Compare 105.0 > 100.0 -> Match
```

**Missing field handling**:

- If `field` missing: Apply `on_missing_field` policy
- If `field_ref` missing: Apply same `on_missing_field` policy
- If either coercion fails: Condition fails (not missing field)

## Null Value Semantics

**Null is always treated as missing field**, regardless of `field_type`:

- `temperature: null` with any `field_type` → Defers to `on_missing_field` setting
- This distinguishes between "field exists with null value" and "field doesn't exist"
- In JSON, both cases result in same behavior (least intrusive default)

**Rationale**: Null semantics vary across languages (Python None, JavaScript null, SQL NULL). Treating null as missing field provides consistent, predictable behavior.

## Type Coercion Failures vs Null-Like Values

**Critical distinction**: Type coercion failures and null-like values are handled differently.

**Type coercion failures** (impossible coercions like `"abc"` -> int/float):

- Treated as **condition failed** (NOT as missing field)
- The condition evaluates to false and evaluation continues
- Does NOT trigger `on_missing_field` policy
- Example: `age: "abc"` with `field_type="int"` and `op: "gt"` -> condition fails, move to next condition/group/rule

**Null-like values** (explicit null values like `{"age": null}` or `{"readings": None}`):

- Treated as **missing field**
- Defers to `on_missing_field` policy setting
- Example: `age: null` with any `field_type` → applies `on_missing_field` behavior

**Rationale**: This distinction enables best-effort evaluation in wildcard arrays. A single unparseable value shouldn't halt processing when other valid values exist. However, explicit null values indicate intentional absence and should respect the user's configured missing field policy.

## Wildcard Array Semantics

Type coercion applies **per-element** when evaluating wildcards. The distinction between coercion failures and null-like values is critical for wildcard behavior.

**Evaluation behavior**:

- Iterate through array elements sequentially
- Apply type coercion to each element independently
- **Impossible coercion** (e.g., `"invalid"` -> int/float): Treat as **condition failed**, continue to next element (NOT missing field)
- **Null-like values** (e.g., `null`, `None`): Treat as **missing field**, defer to `on_missing_field` policy
- Stop on first matching element (short-circuit)

**Example**:

```json
{
  "field": ["readings", "*", "temp"],
  "field_type": "int",
  "op": "gt",
  "value": 15
}
```

Evaluated against: `readings = [{"temp": 10}, {"temp": "invalid"}, {"temp": 30}]`

**Evaluation sequence**:

1. `readings[0].temp` (10) -> integer -> `10 > 15` -> false, continue
2. `readings[1].temp` ("invalid") -> **coercion fails** -> treat as **condition failed** (NOT missing field), continue to next element
3. `readings[2].temp` (30) -> integer -> `30 > 15` -> **true, stop**

**Result**: Rule matches with `matched_value = 30`, `matched_field = ["readings", 2, "temp"]`

**Example with null-like value**: Evaluated against `readings = [{"temp": 10}, {"temp": null}, {"temp": 30}]` with `on_missing_field="skip"`

**Evaluation sequence**:

1. `readings[0].temp` (10) -> integer -> `10 > 15` -> false, continue
2. `readings[1].temp` (null) -> **null-like value** -> treat as **missing field**, apply `on_missing_field` policy (skip), continue to next element
3. `readings[2].temp` (30) -> integer -> `30 > 15` -> **true, stop**

**Result**: Rule matches with `matched_value = 30`, `matched_field = ["readings", 2, "temp"]`

## Performance Optimizations

**Fast-path type checks**:

- Use native type checks before coercion (e.g., type inspection in the target language)
- Avoid string parsing for numeric checks when possible
- Cache parsed numeric values for repeated comparisons

**Sample rate optimization**:

- Skip type coercion entirely when `sample_rate = 0.0` (rule disabled)
- When `sample_rate = 1.0`: Normal coercion path (no RNG overhead)

## Edge Cases and Limitations

**Known Limitations**:

- **Complexity**: Four field types increase cognitive load for users
- **Silent Coercion Failures**: Coercion failures treated as condition failed (not missing field) may silently skip bad data
- **Inconsistent Defaults**: Different operators have different auto-selected types
- **Limited Coercion**: No boolean↔string coercion may surprise JavaScript developers
- **Wildcard Edge Cases**: Per-element coercion in arrays requires careful documentation
- **No Schema Inference**: Users must explicitly specify types (no automatic detection)

**Edge Cases**:

- **Empty strings**: Treated as non-numeric, coercion fails for `field_type="int"` or `field_type="float"`
- **Leading/trailing whitespace**: MUST be trimmed before parsing. All SDKs strip whitespace to ensure consistent coercion behavior across implementations
- **Special float values**: `Infinity`, `NaN` handling depends on language implementation

## Related Documents

**Dependencies** (read these first):

- Rule Expression Language: Defines type system for rule condition evaluation; field type cost multipliers integrate with priority calculation
- Performance Model and Optimization Strategy: Section 4 defines canonical field type multipliers

**Related Spokes** (siblings in this hub):

- Field Path Resolution: Type coercion applies to values resolved through field paths, including cross-field comparisons
- Schema Evolution: Interacts with missing field handling and null value semantics
- Rule Expression Language: Defines how field types (int, float, string, boolean, any) integrate with operators and conditions

**Extended by** (documents building on this):

- Unified Validation and Input Sanitization: Type coercion validation (int/float/string/boolean coercion, null vs coercion failure distinction) consolidated
