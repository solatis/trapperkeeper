# ADR-017: Schema Evolution and Missing Field Handling

## Revision log

| Date | Description |
|------|-------------|
| 2025-10-28 | Document created |

## Context

TrapperKeeper operates on arbitrary data schemas that evolve over time without pre-registration. Data pipelines frequently encounter schema changes:

- **New fields added**: Upstream systems add new attributes without coordination
- **Fields removed**: Deprecated fields disappear from new records
- **Field type changes**: Numeric IDs converted to strings, booleans to integers
- **Null values**: Fields present but null in some records
- **Schema-agnostic architecture**: Server has zero understanding of schemas (see ADR-001)

Traditional schema registries solve this with centralized coordination, but require pre-registration and centralized schema management. TrapperKeeper's schema-agnostic design (no pre-registration, no schema registry) requires explicit strategies for handling schema evolution.

Rules that reference missing fields create ambiguous situations:
- Should a rule checking `customer.age > 18` match when `age` field is missing?
- Should pipeline fail-fast or continue processing?
- How should null values be distinguished from missing fields?

## Decision

We will implement **explicit missing field handling** with three configurable modes that put control in the rule author's hands.

### 1. Missing Field Configuration

Every rule has an `on_missing_field` configuration option (per-rule, not per-condition):

| Mode | Behavior | Use Case |
|------|----------|----------|
| `skip` | Field missing → rule doesn't match, continue to next rule | Default, least intrusive |
| `match` | Field missing → treat as matching condition | Detect incomplete records |
| `error` | Field missing → raise exception, fail pipeline | Strict validation |

**Default**: `skip` (least intrusive by default, aligns with ADR-001 principles)

**Rationale**: Explicit configuration prevents implicit assumptions. Different use cases require different behaviors - data quality monitoring needs "match" mode, production pipelines need "skip" mode. See Appendix A for configuration examples.

### 2. Null Value Semantics

**Null values treated as missing field**:
- JSON: `{"customer": {"age": null}}`
- Behavior: Same as if `age` key didn't exist
- Defers to `on_missing_field` policy

**Rationale**: Distinguishing "field present but null" from "field absent" adds implementation complexity without clear benefit. Most languages (Python, JavaScript) treat null/undefined similarly in conditionals. Unified handling simplifies SDK implementation.

### 3. Schema Evolution Patterns

#### New Fields Added
**Behavior**: Existing rules unaffected unless they explicitly reference the new field

**Example**:
- Original schema: `{"name": "Alice", "age": 30}`
- New schema: `{"name": "Alice", "age": 30, "email": "alice@example.com"}`
- Rule checking `age > 18`: Continues matching as before
- Rule checking `email` field: Matches new records, behavior on old records depends on `on_missing_field`

**Rationale**: Additive changes are safe. Rules only evaluate fields they explicitly reference.

#### Fields Removed
**Behavior**: Depends on `on_missing_field` setting

**Example**:
- Original schema: `{"name": "Alice", "age": 30, "department": "Engineering"}`
- New schema: `{"name": "Alice", "age": 30}` (department removed)
- Rule checking `department == "Engineering"`:
  - `on_missing_field="skip"`: Rule doesn't match (default)
  - `on_missing_field="match"`: Rule matches (detects removal)
  - `on_missing_field="error"`: Pipeline fails

**Rationale**: Breaking changes require explicit choice. Default "skip" prevents pipeline failures.

#### Field Type Changes
**Behavior**: Handled by type coercion system (see ADR-001)

**Example**:
- Original schema: `{"user_id": 12345}` (numeric)
- New schema: `{"user_id": "12345"}` (string)
- Rule with `field_type="numeric"`: Coerces `"12345"` → `12345`
- Rule with `field_type="text"`: Coerces `12345` → `"12345"`

**Coercion failure behavior**:
- If coercion impossible (e.g., `"abc"` to numeric): Treated as condition failed
- Wrapped in `on_missing_field` logic:
  - `on_missing_field="skip"`: Skip rule, continue
  - `on_missing_field="error"`: Raise exception

**Rationale**: Lenient type coercion handles gradual migrations. Strict mode available via `on_missing_field="error"` when needed.

### 4. Interaction with Field Path Resolution

**Nested paths**: Missing field handling applies at any depth

**Scenarios**:
- `customer` field missing → `on_missing_field` policy applies
- `address` field missing from `customer` → `on_missing_field` policy applies
- `zipcode` field missing from `address` → `on_missing_field` policy applies

**Wildcard paths**: Missing field handling per array element (see ADR-015)

**Behavior**:
- `readings[0].temp` missing → Skip element, try `readings[1]`
- `readings[1].temp = 105` → Match, return this value
- If all elements missing `temp` → `on_missing_field` policy applies to entire condition

**Rationale**: Per-element handling enables resilient evaluation over heterogeneous arrays. First-match short-circuit stops at first valid match. See Appendix B for nested path examples.

### 5. Interaction with Type System

Missing field handling integrates with type coercion (ADR-016):

**Processing order**:
1. Resolve field path → Field value or "missing"
2. If missing → Apply `on_missing_field` policy
3. If present → Attempt type coercion based on `field_type`
4. If coercion fails → Treat as condition failed, wrap in `on_missing_field` logic
5. If coercion succeeds → Evaluate operator

**Rationale**: Unified error handling. Type coercion failures behave consistently with missing fields. See Appendix C for detailed processing examples.

## Consequences

### Benefits

1. **Explicit Control**: Rule authors choose missing field behavior per use case
2. **Safe Defaults**: `skip` mode prevents pipeline failures from schema drift
3. **Data Quality Monitoring**: `match` mode enables detection of incomplete records
4. **Strict Validation**: `error` mode enforces schema contracts when needed
5. **Gradual Migration**: Type coercion handles field type changes during transitions
6. **Null Handling Simplicity**: Unified null/missing semantics reduces edge cases
7. **Array Resilience**: Per-element handling enables robust wildcard evaluation

### Tradeoffs

1. **No Schema Validation**: System cannot detect schema errors before evaluation
2. **Duplicate Behavior**: Type coercion failures and missing fields handled similarly (intentional simplicity)
3. **No "Field Present" Check**: Cannot distinguish null from missing without explicit `exists`/`is_null` operators
4. **Migration Visibility**: Silent schema changes with `skip` mode may hide data quality issues
5. **Per-Rule Configuration**: Cannot set different policies per condition within same rule

### Operational Implications

1. **Rule Migration**: When schemas change, review rules referencing removed/changed fields
2. **Monitoring**: Track rules that frequently skip due to missing fields
3. **Testing Strategy**: Test rules against both old and new schemas during migrations
4. **Documentation**: Rule descriptions should document expected schema
5. **Gradual Rollout**: Use `skip` mode initially, switch to `error` after stabilization

## Implementation

1. Extend rule schema with `on_missing_field` field:
   - Add enum column to rules table: `on_missing_field TEXT NOT NULL DEFAULT 'skip'`
   - Values: `skip`, `match`, `error`
   - Validation: Check enum values on rule creation/update

2. Implement field resolution in SDKs:
   - Distinguish "field missing" from "field present but null"
   - Return sentinel value indicating missing field
   - Propagate through nested path resolution

3. Implement missing field handling:
   - After field resolution, check for missing sentinel
   - Apply `on_missing_field` policy:
     - `skip`: Return false (condition doesn't match)
     - `match`: Return true (condition matches)
     - `error`: Raise exception with field path context

4. Integrate with type coercion (ADR-016):
   - If coercion fails, wrap in `on_missing_field` logic
   - Treat coercion failure as missing field for policy purposes
   - Log warning in debug mode

5. Extend wildcard evaluation (ADR-015):
   - Check each array element for missing field
   - Skip elements where field missing
   - Apply `on_missing_field` only if all elements missing field
   - Record matched element index in event

6. Update UI rule builder:
   - Add dropdown for `on_missing_field` setting
   - Default to `skip` with help text explaining modes
   - Show warning when selecting `error` mode
   - Include examples for each mode

7. Add monitoring:
   - Log rules that skip due to missing fields (debug level)
   - Track metrics: Rules skipped per missing field
   - Alert if skip rate exceeds threshold (indicates schema drift)

## Related Decisions

This ADR defines how TrapperKeeper handles schema evolution and missing fields through configurable policies that integrate with field path resolution and type coercion.

**Depends on**:
- **ADR-015: Field Path Resolution** - Provides runtime field path resolution mechanism extended with missing field handling
- **ADR-016: Type System and Coercion** - Defines type coercion rules that interact with missing field semantics

## Future Considerations

- **Per-Condition Configuration**: Allow different policies per condition within same rule
- **Schema Change Detection**: Alert when field access patterns change significantly
- **Schema History**: Track field presence statistics over time
- **Automatic Schema Inference**: Suggest rule updates when schemas evolve
- **Dry-Run Mode**: Test rules against new schemas before deployment
- **Field Deprecation Warnings**: Detect when rules reference rarely-present fields

## Appendix A: Missing Field Configuration Examples

**Example 1: Detect missing age field (match mode)**
```json
{
  "rule_id": "01936a3e-1234-7b3c-9d5e-abcdef123456",
  "name": "Detect missing age field",
  "action": "observe",
  "on_missing_field": "match",
  "any": [
    {
      "all": [
        {
          "field": ["customer", "age"],
          "field_type": "numeric",
          "op": "exists",
          "value": null
        }
      ]
    }
  ]
}
```

**Example 2: Skip records without email (skip mode - default)**
```json
{
  "rule_id": "01936a3e-5678-7b3c-9d5e-abcdef789012",
  "name": "Validate email format",
  "action": "observe",
  "on_missing_field": "skip",
  "any": [
    {
      "all": [
        {
          "field": ["contact", "email"],
          "field_type": "text",
          "op": "regex",
          "value": "^[\\w.-]+@[\\w.-]+\\.[a-zA-Z]{2,}$"
        }
      ]
    }
  ]
}
```

**Example 3: Enforce required fields (error mode)**
```json
{
  "rule_id": "01936a3e-9abc-7b3c-9d5e-abcdef345678",
  "name": "Require user_id field",
  "action": "observe",
  "on_missing_field": "error",
  "any": [
    {
      "all": [
        {
          "field": ["user_id"],
          "field_type": "text",
          "op": "exists",
          "value": null
        }
      ]
    }
  ]
}
```

## Appendix B: Nested Path Resolution Examples

**Example 1: Nested object path**
```json
{
  "field": ["customer", "address", "zipcode"],
  "op": "eq",
  "value": "94102"
}
```

**Data scenarios**:
```json
// Scenario 1: Complete path
{"customer": {"address": {"zipcode": "94102"}}}
// Result: Match

// Scenario 2: Missing zipcode
{"customer": {"address": {}}}
// Result: on_missing_field policy applies

// Scenario 3: Missing address
{"customer": {}}
// Result: on_missing_field policy applies

// Scenario 4: Missing customer
{}
// Result: on_missing_field policy applies
```

**Example 2: Wildcard array path**
```json
{
  "field": ["readings", "*", "temp"],
  "op": "gt",
  "value": 100
}
```

**Data scenarios**:
```json
// Scenario 1: First element has temp
{"readings": [{"temp": 105}, {"temp": 95}]}
// Result: Match (returns 105 from first element)

// Scenario 2: First element missing temp, second has it
{"readings": [{"pressure": 30}, {"temp": 105}]}
// Result: Match (skips first, returns 105 from second)

// Scenario 3: All elements missing temp
{"readings": [{"pressure": 30}, {"pressure": 28}]}
// Result: on_missing_field policy applies to entire condition

// Scenario 4: Empty array
{"readings": []}
// Result: on_missing_field policy applies
```

## Appendix C: Type Coercion Processing Examples

**Processing flow for `field_type="numeric"` condition**:
```json
{
  "field": ["temperature"],
  "field_type": "numeric",
  "op": "gt",
  "value": 100
}
```

| Data | Step 1: Resolve | Step 2: Missing Check | Step 3: Coercion | Step 4: Evaluate | Final Result |
|------|-----------------|----------------------|------------------|------------------|--------------|
| `{}` | Field missing | `on_missing_field` applies | — | — | Depends on policy (`skip`/`match`/`error`) |
| `{"temperature": null}` | null → treated as missing | `on_missing_field` applies | — | — | Depends on policy |
| `{"temperature": 105}` | Field present: 105 | Pass | 105 (already numeric) | `105 > 100` → true | **Match** |
| `{"temperature": "105"}` | Field present: "105" | Pass | "105" → 105 (coerce success) | `105 > 100` → true | **Match** |
| `{"temperature": "105.5"}` | Field present: "105.5" | Pass | "105.5" → 105.5 (coerce success) | `105.5 > 100` → true | **Match** |
| `{"temperature": "abc"}` | Field present: "abc" | Pass | "abc" → fail | Coercion failure | Depends on policy (treated as missing) |
| `{"temperature": true}` | Field present: true | Pass | true → 1 (coerce success) | `1 > 100` → false | **No match** |
| `{"temperature": false}` | Field present: false | Pass | false → 0 (coerce success) | `0 > 100` → false | **No match** |

**Configuration mode implications**:

**Mode: `skip` (default)**
- Missing field or coercion failure: Rule doesn't match, continue to next rule
- Use case: Production pipelines that should tolerate schema drift

**Mode: `match`**
- Missing field or coercion failure: Rule matches
- Use case: Data quality monitoring to detect incomplete records

**Mode: `error`**
- Missing field or coercion failure: Raise exception, fail pipeline
- Use case: Strict validation where schema contract must be enforced
