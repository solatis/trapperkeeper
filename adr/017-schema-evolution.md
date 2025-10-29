# ADR-017: Schema Evolution and Missing Field Handling
Date: 2025-10-28

## Context

TrapperKeeper operates on arbitrary data schemas that evolve over time without pre-registration. Data pipelines frequently encounter schema changes:

- **New fields added**: Upstream systems add new attributes without coordination
- **Fields removed**: Deprecated fields disappear from new records
- **Field type changes**: Numeric IDs converted to strings, booleans to integers
- **Null values**: Fields present but null in some records
- **Schema-agnostic architecture**: Server has zero understanding of schemas (see ADR-023)

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

**Default**: `skip` (least intrusive by default, aligns with ADR-023 principles)

**Example**:
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

**Rationale**: Explicit configuration prevents implicit assumptions. Different use cases require different behaviors - data quality monitoring needs "match" mode, production pipelines need "skip" mode.

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
**Behavior**: Handled by type coercion system (see ADR-016)

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

**Example**:
```json
{
  "field": ["customer", "address", "zipcode"],
  "op": "eq",
  "value": "94102"
}
```

**Scenarios**:
- `customer` field missing → `on_missing_field` policy applies
- `address` field missing from `customer` → `on_missing_field` policy applies
- `zipcode` field missing from `address` → `on_missing_field` policy applies

**Wildcard paths**: Missing field handling per array element (see ADR-015)

**Example**:
```json
{
  "field": ["readings", "*", "temp"],
  "op": "gt",
  "value": 100
}
```

**Behavior**:
- `readings[0].temp` missing → Skip element, try `readings[1]`
- `readings[1].temp = 105` → Match, return this value
- If all elements missing `temp` → `on_missing_field` policy applies to entire condition

**Rationale**: Per-element handling enables resilient evaluation over heterogeneous arrays. First-match short-circuit stops at first valid match.

### 5. Interaction with Type System

Missing field handling integrates with type coercion (ADR-016):

**Processing order**:
1. Resolve field path → Field value or "missing"
2. If missing → Apply `on_missing_field` policy
3. If present → Attempt type coercion based on `field_type`
4. If coercion fails → Treat as condition failed, wrap in `on_missing_field` logic
5. If coercion succeeds → Evaluate operator

**Example with `field_type="numeric"`**:
```json
{
  "field": ["temperature"],
  "field_type": "numeric",
  "op": "gt",
  "value": 100
}
```

| Data | Step 1 | Step 2 | Step 3 | Result |
|------|--------|--------|--------|--------|
| `{}` | missing | `on_missing_field` applies | — | Depends on policy |
| `{"temperature": null}` | missing (null → missing) | `on_missing_field` applies | — | Depends on policy |
| `{"temperature": 105}` | present | — | Coerce: 105 (success) | Evaluate: `105 > 100` → match |
| `{"temperature": "105"}` | present | — | Coerce: `"105"` → 105 | Evaluate: `105 > 100` → match |
| `{"temperature": "abc"}` | present | — | Coerce: fail | Treat as failed, `on_missing_field` applies |

**Rationale**: Unified error handling. Type coercion failures behave consistently with missing fields.

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

**Depends on:**
- **ADR-015: Field Path Resolution** - Extends field resolution with explicit missing field handling modes
- **ADR-016: Type System and Coercion** - Defines interaction between null values and missing fields

## Future Considerations

- **Per-Condition Configuration**: Allow different policies per condition within same rule
- **Schema Change Detection**: Alert when field access patterns change significantly
- **Schema History**: Track field presence statistics over time
- **Automatic Schema Inference**: Suggest rule updates when schemas evolve
- **Dry-Run Mode**: Test rules against new schemas before deployment
- **Field Deprecation Warnings**: Detect when rules reference rarely-present fields
