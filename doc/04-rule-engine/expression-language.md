---
doc_type: spoke
status: active
primary_category: architecture
hub_document: doc/04-rule-engine/README.md
tags:
  - expression-language
  - dnf
  - operators
  - rule-schema
cross_cutting:
  - validation
---

# Rule Expression Language

## Context

TrapperKeeper rules must be expressive enough for complex boolean logic while remaining simple enough for non-programmers to construct through a visual UI. The expression system must optimize for <1ms evaluation time, support pre-compilation for performance, and provide clear first-match semantics without ambiguity.

**Hub Document**: This document is part of the Rule Engine Architecture. See [Rule Engine Architecture](README.md) for strategic overview and relationships to field resolution, type coercion, and operational controls.

## DNF Schema Structure

Rules use OR-of-ANDs structure represented as JSON:

```json
{
  "version": 1,
  "rule_id": "01936a3e-1234-7b3c-9d5e-abcdef123456",
  "name": "Invalid temperature check",
  "description": "Drop records with temperature outside valid range or from faulty sensors",
  "action": "drop",
  "sample_rate": 1.0,
  "scope": {
    "tags": ["production", "customer-data"]
  },
  "any": [
    {
      "all": [
        {
          "field": ["temperature"],
          "field_type": "numeric",
          "op": "gt",
          "value": 100,
          "on_missing_field": "skip"
        },
        {
          "field": ["sensor_id"],
          "field_type": "text",
          "op": "prefix",
          "value": "TEMP-",
          "on_missing_field": "skip"
        }
      ]
    },
    {
      "all": [
        {
          "field": ["temperature"],
          "field_type": "numeric",
          "op": "lt",
          "value": 0,
          "on_missing_field": "skip"
        }
      ]
    }
  ]
}
```

**Evaluation semantics**: Match if **ANY** group matches, where a group matches if **ALL** conditions match.

Above example: Match if `(temperature > 100 AND sensor_id starts with "TEMP-") OR (temperature < 0)`

**Rationale**: DNF structure maps naturally to visual UI (tabs for OR groups, lists for AND conditions). Pre-compilation to nested predicates enables short-circuit evaluation. No runtime parsing or expression trees.

## Rule Fields

| Field         | Type     | Required | Description                                                                                  |
| ------------- | -------- | -------- | -------------------------------------------------------------------------------------------- |
| `version`     | int      | Yes      | Schema version (currently 1) for forward compatibility                                       |
| `rule_id`     | UUIDv7   | Yes      | Server-generated identifier, never user-provided                                             |
| `name`        | string   | Yes      | User-provided name, 1-128 chars, UTF-8, **not unique** (duplicates allowed like AWS console) |
| `description` | string   | No       | Plain text description, 1-1024 chars, UTF-8, newline/tabs ok, no markdown                    |
| `action`      | enum     | Yes      | `"observe"`, `"drop"`, or `"error"`                                                          |
| `sample_rate` | float    | No       | Default 1.0 (100%), range [0.0, 1.0], applied before field extraction                        |
| `scope.tags`  | string[] | Yes      | Which sensors this rule applies to                                                           |
| `any`         | array    | Yes      | OR groups (minimum one group)                                                                |
| `any[].all`   | array    | Yes      | AND conditions (minimum one condition per group)                                             |

**Key design notes**:

- **`rule_id` is UUIDv7**: Use native UUID database type where supported (PostgreSQL, MySQL) for performance
- **No priority field**: Priority dynamically calculated by API server based on rule complexity, not persisted in database
- **Name duplication**: Allowed intentionally to match user expectations from AWS/GCP consoles
- **Sample rate optimization**: When `sample_rate` is exactly 0.0 or 1.0, skip random number generation entirely

## Condition Schema

Each condition in an `all` array has this structure:

```json
{
  "field": ["path", "to", "field"],
  "field_type": "numeric",
  "op": "gt",
  "value": 100,
  "on_missing_field": "skip",
  "on_coercion_fail": "skip"
}
```

**Field definitions**:

- **`field`**: Array of path components (strings or integers) representing nested access. See Field Path Resolution for complete semantics including wildcard handling.
- **`field_type`**: Per-condition type annotation (`"numeric"`, `"text"`, `"boolean"`, `"any"`). Different conditions in same rule can have different types. See Type System and Coercion for coercion rules.
- **`op`**: Operator from supported set (see below).
- **`value`**: Comparison value (string, number, boolean, or null depending on operator). Omitted or ignored for `is_null` and `exists` operators. **Mutually exclusive with `field_ref`**.
- **`values`**: Array of comparison values for `in` operator only (1-64 elements, all same type). **Not used with other operators**.
- **`field_ref`**: Array path to another field in same record for cross-field comparisons. **Mutually exclusive with `value`**. **No wildcards allowed**.
- **`on_missing_field`**: Enum (`"skip"`, `"match"`, `"error"`), **required**, default `"skip"`
  - `"skip"`: Field missing → condition doesn't match, continue evaluation (least intrusive)
  - `"match"`: Field missing → treat as matching condition (detect incomplete records)
  - `"error"`: Field missing → raise exception, fail pipeline (strict validation)
  - **Default**: `"skip"` (aligns with Least Intrusive by Default principle)
  - **Database**: NOT NULL with default value
  - See Schema Evolution for complete missing field handling semantics
- **`on_coercion_fail`**: Enum (`"skip"`, `"match"`, `"error"`), **required**, default `"skip"`
  - `"skip"`: Coercion fails → condition doesn't match, continue evaluation (best-effort parsing)
  - `"match"`: Coercion fails → treat as matching condition (detect bad data)
  - `"error"`: Coercion fails → raise exception, fail pipeline (strict type validation)
  - **Default**: `"skip"` (aligns with best-effort evaluation principle)
  - **Database**: NOT NULL with default value
  - See Schema Evolution for complete coercion failure handling semantics

**Rationale**: Per-condition `field_type` enables mixed-type rules (e.g., numeric comparison AND string prefix check). Array-based field paths enable natural JSON serialization of complex paths with wildcards and array indices. Per-condition `on_missing_field` enables fine-grained missing field handling within a single rule.

### Cross-Field Comparisons with field_ref

Rules can compare two fields in the same record using `field_ref`:

**Schema**:

```json
{
  "field": ["temperature"],
  "field_type": "numeric",
  "op": "gt",
  "field_ref": ["calibrated_max"]
}
```

**Semantics**:

- Condition specifies either `value` OR `field_ref`, not both
- `field_ref` uses same array path notation as `field` (see Field Path Resolution)
- **No wildcards allowed** in `field_ref` - must resolve to single value
- If `field_ref` path not found, applies `on_missing_field` behavior
- Type coercion applies to both `field` and `field_ref` values (see Type System and Coercion)
- Lookup cost: `field_ref` path evaluated same as `field` path (128 per string component)

**Validation**:

- UI: Make wildcards impossible in field_ref input
- API: Validate field_ref contains no wildcards on rule creation
- Runtime: Enforce no wildcards, return error if found

**Example use case**:

```json
// Industrial IoT: Compare sensor reading against calibrated threshold
{
  "field": ["reading_value"],
  "field_type": "numeric",
  "op": "gt",
  "field_ref": ["calibrated_max"]
}

// Evaluates against: {"reading_value": 105, "calibrated_max": 100}
// Result: 105 > 100 → Match
```

## Supported Operators

MVP operator set optimized for performance (no regex):

| Category   | Operator  | Description             | Required field_type           |
| ---------- | --------- | ----------------------- | ----------------------------- |
| Equality   | `eq`      | Equal                   | User-selected via UI dropdown |
| Equality   | `neq`     | Not equal               | User-selected via UI dropdown |
| Comparison | `lt`      | Less than               | `numeric` only                |
| Comparison | `lte`     | Less than or equal      | `numeric` only                |
| Comparison | `gt`      | Greater than            | `numeric` only                |
| Comparison | `gte`     | Greater than or equal   | `numeric` only                |
| String     | `prefix`  | Starts with             | `any`, `text` (auto-coercion) |
| String     | `suffix`  | Ends with               | `any`, `text` (auto-coercion) |
| Set        | `in`      | Value in set            | User-selected via UI dropdown |
| Existence  | `is_null` | Field is null           | Any type, `value` ignored     |
| Existence  | `exists`  | Field exists (non-null) | Any type, `value` ignored     |

**Operator selection rules**:

- **Comparison operators** (`gt`, `gte`, `lt`, `lte`) always require `field_type = "numeric"` with strict type checking.
- **String operators** (`prefix`, `suffix`) work with `field_type = "any"` or `field_type = "text"`. All values are automatically coerced to string for comparison, including numeric types (int/float) and boolean. **Note**: Boolean values coerce to "true" or "false" strings, making prefix/suffix operations technically supported but not practical for boolean fields.
- **Equality operators** (`eq`, `neq`) require explicit UI selection of `field_type` with dropdown and "strict type matching" checkbox. No default provided; forces user awareness.
- **Set operator** (`in`) requires explicit UI selection of `field_type` with dropdown. Uses `values` array field instead of single `value`.
- **Existence operators** (`is_null`, `exists`) work with any `field_type` and ignore `value` field.

**Why no regex**: Regular expressions too slow for high-throughput evaluation. String prefix/suffix operations optimize to simple byte comparisons.

### IN Operator Semantics

The `in` operator checks if a field value exists in a set of values.

**Schema**:

```json
{
  "field": ["status"],
  "field_type": "text",
  "op": "in",
  "values": ["active", "pending", "processing"]
}
```

**Rules**:

- Uses `values` field (array) instead of `value` (single value)
- Maximum 64 elements in `values` array
- All values in array must be same type
- Type coercion applies to field value (see Type System and Coercion)
- Validation: UI prevents invalid arrays, API validates on rule creation, runtime enforces constraints

**Example**:

```json
{
  "name": "Valid statuses",
  "any": [
    {
      "all": [
        {
          "field": ["status"],
          "field_type": "text",
          "op": "in",
          "values": ["active", "pending", "processing"],
          "on_missing_field": "skip"
        }
      ]
    }
  ]
}
```

**Rationale**: Set membership checks are common in data quality rules (status validation, category filtering, allowlist/blocklist). The `in` operator provides cleaner syntax than multiple `eq` conditions in OR groups and enables future optimizations like bloom filters or hash sets.

## UI Design Implications

Visual rule builder structure maps directly to DNF schema:

**Top level**: "Match ANY of these groups"

- Rendered as tabs or expandable sections
- Each tab = one `any[]` group
- "Add Group" button creates new OR condition

**Each group**: "Match ALL of these conditions"

- Rendered as vertical list of condition rows
- Each row = one condition in `all[]` array
- "Add Condition" button adds new AND clause

**Condition builder**:

- Field selector: Manual text input for field paths (e.g., `["temperature"]`, `["sensors", "*", "value"]`)
- Operator dropdown: Available operators depend on selected field type
- Field type selector: Auto-selected for comparison/string operators, explicit dropdown for equality
- Value input: Type-appropriate widget (number input, text input, checkbox)

**No JSON editing required**: All rule construction through visual components. Advanced mode may expose JSON for power users, but not required for basic usage.

**Rationale**: DNF structure prevents users from creating unparseable boolean expressions. Visual builder eliminates syntax errors. Direct correspondence between UI and schema simplifies implementation.

## Matched Field and Value Resolution

When a rule matches, the system records three pieces of diagnostic information in the event:

- **`matched_field`**: The resolved field path that triggered the match (wildcards resolved to actual indices)
- **`matched_value`**: The actual value that satisfied the condition
- **`matched_condition`**: Path to the specific condition that triggered the match

### matched_condition Format

Array notation identifying the matched condition's location in the rule structure:

```json
["any", <group_index>, "all"]
```

**Semantics**:

- The `"all"` indicates all conditions in the group matched (AND semantics require all to be true)
- No individual condition index needed since ALL conditions must have matched
- Short-circuit evaluation uses first matching group
- This format enables precise identification of which OR group triggered the rule

**Rationale**: The `"all"` marker confirms that every condition in the group matched. Since AND semantics require all conditions to be true, there is no single "triggering" condition—the entire group succeeded together.

### Examples

**Single Group Match**:

```json
// Rule structure:
{
  "any": [
    {
      "all": [
        {"field": ["temp"], "op": "gt", "value": 100}
      ]
    },
    {
      "all": [
        {"field": ["pressure"], "op": "lt", "value": 10}
      ]
    }
  ]
}

// Event: {"temp": 50, "pressure": 8}
// Evaluation: Group 0 fails (temp not > 100), Group 1 matches (pressure < 10)

// Event output:
{
  "matched_field": ["pressure"],
  "matched_value": 8,
  "matched_condition": ["any", 1, "all"]  // Second OR group matched
}
```

**Multi-Group Potential Match with Short-Circuit**:

When short-circuit evaluation is used, only the **first matching group** is returned:

```json
// Event: {"temp": 105, "pressure": 8}
// Both groups would match, but evaluation stops at first match

// Event output:
{
  "matched_field": ["temp"],
  "matched_value": 105,
  "matched_condition": ["any", 0, "all"] // First group matched, evaluation stopped
}
```

**Multi-Condition Group**:

```json
// Rule structure:
{
  "any": [
    {
      "all": [
        {"field": ["temp"], "op": "gt", "value": 100},
        {"field": ["sensor_id"], "op": "prefix", "value": "TEMP-"}
      ]
    }
  ]
}

// Event: {"temp": 105, "sensor_id": "TEMP-001"}
// Both conditions in group 0 match

// Event output:
{
  "matched_field": ["temp"],  // First condition's field
  "matched_value": 105,       // First condition's value
  "matched_condition": ["any", 0, "all"]  // Group 0 matched, all conditions satisfied
}
```

**Note**: When multiple conditions in an `all` group match, `matched_field` and `matched_value` report the **first matching condition** in evaluation order (per cost-based ordering). The `matched_condition` confirms that all conditions in the group passed.

**Cross-SDK consistency note**: Because short-circuit evaluation is an optional optimization, different SDK implementations may report different `matched_field` and `matched_value` values for the same input, particularly when:

- Multiple conditions in an `all` group could match
- Wildcard arrays have multiple matching elements
- Different `any` groups could match

This variance is **acceptable and expected**. The semantic result (match/no-match) must be identical across implementations, but the specific diagnostic fields may differ based on evaluation strategy.

## Wire Protocol vs Documentation Format

**Critical distinction**:

- **This document uses JSON** for human readability
- **Implementation uses Protocol Buffers** for performance and type safety
- JSON examples represent logical schema, not actual wire format

**Protobuf benefits**:

- Compile-time type checking prevents schema errors
- Binary encoding reduces bandwidth for rule sync
- Code generation ensures type-safe SDK implementations
- Schema evolution with backward compatibility guarantees

**Protocol Buffer Definitions**: Complete protobuf schemas defined in `proto/trapperkeeper/sensor/v1/`. Key design decisions:

- Typed field path components: `PathComponent` uses `oneof` to discriminate string keys, integer indices, and wildcards
- Package namespace: `trapperkeeper.sensor.v1` enables future versioning

## Edge Cases and Limitations

**Known Limitations**:

- **No regex support**: String matching limited to prefix/suffix operators. Cannot validate email formats, parse structured strings, or match complex patterns.
- **DNF constraints**: Cannot express `NOT (A OR B)` without De Morgan transformation. Users must manually convert `NOT (A AND B)` to `(NOT A) OR (NOT B)` using `neq` operators.
- **Wildcard ANY semantics only**: Cannot express "ALL elements must satisfy" without multiple conditions.
- **No cross-field wildcard comparisons**: `field_ref` cannot use wildcards, limiting cross-field comparisons to scalar fields only.
- **IN operator cardinality limit**: Maximum 64 values in `values` array. Large sets require alternative approaches.

**Edge Cases**:

- **Empty any[] or all[] arrays**: Validation rejects rules with empty OR groups or AND conditions
- **Name uniqueness**: Duplicate names allowed intentionally. Users rely on descriptions and tags for organization.
- **Sample rate edge values**: 0.0 and 1.0 use fast paths (no RNG). Values between use cryptographically secure RNG.

## Related Documents

**Dependencies** (read these first):

- Architectural Principles: Implements Schema-Agnostic Architecture through runtime field resolution
- Performance Model and Optimization Strategy: Canonical cost calculation for rule priority

**Related Spokes** (siblings in this hub):

- Field Path Resolution: Defines the field path notation and wildcard semantics used in condition `field` arrays
- Type System and Coercion: Specifies type handling for rule evaluation based on `field_type`
- Schema Evolution: Handles missing fields using `on_missing_field` policy
- Lifecycle: Adds operational controls for rules (dry-run, pause, enable/disable)

**Extended by** (documents building on this):

- Sampling and Performance Optimization: Extends rules with sample_rate field and cost-based predicate ordering
- Batch Processing and Vectorization: Defines vectorized rule evaluation for Pandas/Spark frameworks
- Unified Validation and Input Sanitization: Rule expression validation consolidated (operator/field_type compatibility, nested wildcard limits, sample_rate range validation)
