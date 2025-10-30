# ADR-014: Rule Expression Language and Schema

## Revision log

| Date | Description |
|------|-------------|
| 2025-10-28 | Document created |

## Context

TrapperKeeper's rule engine evaluates data quality rules against streaming records with strict performance requirements. The rule expression system must:

- **Performance**: Evaluate rules in <1ms per record for high-throughput pipelines
- **Usability**: Enable non-programmers to build rules through a visual UI
- **Expressiveness**: Support complex boolean logic without sacrificing performance
- **Predictability**: Firewall-style sequential evaluation with clear first-match semantics
- **Safety**: Explicit handling of missing fields, type mismatches, and schema evolution
- **Pre-compilation**: Optimize rules once at sync time, not per-record

This ADR formalizes the complete rule schema, operator set, and evaluation semantics.

## Decision

We will implement a **DNF (Disjunctive Normal Form)** rule expression system optimized for pre-compilation and UI construction.

### 1. DNF Schema Structure

Rules use OR-of-ANDs structure represented as JSON (documentation format; wire protocol uses Protocol Buffers):

```json
{
  "version": 1,
  "rule_id": "01936a3e-1234-7b3c-9d5e-abcdef123456",
  "name": "Invalid temperature check",
  "description": "Drop records with temperature outside valid range or from faulty sensors",
  "action": "drop",
  "sample_rate": 1.0,
  "on_missing_field": "skip",
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
          "value": 100
        },
        {
          "field": ["sensor_id"],
          "field_type": "text",
          "op": "prefix",
          "value": "TEMP-"
        }
      ]
    },
    {
      "all": [
        {
          "field": ["temperature"],
          "field_type": "numeric",
          "op": "lt",
          "value": 0
        }
      ]
    }
  ]
}
```

**Evaluation semantics**: Match if **ANY** group matches, where a group matches if **ALL** conditions match.

Above example: Match if `(temperature > 100 AND sensor_id starts with "TEMP-") OR (temperature < 0)`

**Rationale**: DNF structure maps naturally to visual UI (tabs for OR groups, lists for AND conditions). Pre-compilation to nested predicates enables short-circuit evaluation. No runtime parsing or expression trees.

### 2. Rule Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `version` | int | Yes | Schema version (currently 1) for forward compatibility |
| `rule_id` | UUIDv7 | Yes | Server-generated identifier, never user-provided |
| `name` | string | Yes | User-provided name, 1-128 chars, UTF-8, **not unique** (duplicates allowed like AWS console) |
| `description` | string | No | Plain text description, 1-1024 chars, UTF-8, newline/tabs ok, no markdown |
| `action` | enum | Yes | `"observe"`, `"drop"`, or `"error"` |
| `sample_rate` | float | No | Default 1.0 (100%), range [0.0, 1.0], applied before field extraction |
| `on_missing_field` | enum | No | Default `"skip"`: one of `"skip"`, `"match"`, `"error"` (see ADR-016) |
| `scope.tags` | string[] | Yes | Which sensors this rule applies to (see ADR-001) |
| `any` | array | Yes | OR groups (minimum one group) |
| `any[].all` | array | Yes | AND conditions (minimum one condition per group) |

**Key design notes**:

- **`rule_id` is UUIDv7**: Use native UUID database type where supported (PostgreSQL, MySQL) for performance. See ADR-007.
- **No priority field**: Priority dynamically calculated by API server based on rule complexity, not persisted in database. Calculation considers condition count, operator cost, and sampling rate.
- **Tenant isolation**: `tenant_id` (UUIDv7) present in multi-tenant deployments, though MVP is single-tenant only.
- **Name duplication**: Allowed intentionally to match user expectations from AWS/GCP consoles. Users rely on descriptions and tags for organization.
- **Sample rate optimization**: When `sample_rate` is exactly 0.0 or 1.0, skip random number generation entirely. Apply sampling before expensive field extraction.

### 3. Condition Schema

Each condition in an `all` array has this structure:

```json
{
  "field": ["path", "to", "field"],
  "field_type": "numeric",
  "op": "gt",
  "value": 100
}
```

**Field definitions**:

- **`field`**: Array of path components (strings or integers) representing nested access. See ADR-015 for complete field path resolution semantics including wildcard handling.
- **`field_type`**: Per-condition type annotation (`"numeric"`, `"text"`, `"boolean"`, `"any"`). Different conditions in same rule can have different types. See ADR-016 for type coercion rules.
- **`op`**: Operator from supported set (see below).
- **`value`**: Comparison value (string, number, boolean, or null depending on operator). Omitted or ignored for `is_null` and `exists` operators.

**Rationale**: Per-condition `field_type` enables mixed-type rules (e.g., numeric comparison AND string prefix check). Array-based field paths enable natural JSON serialization of complex paths with wildcards and array indices.

### 4. Supported Operators

MVP operator set optimized for performance (no regex):

| Category | Operator | Description | Required field_type |
|----------|----------|-------------|---------------------|
| Equality | `eq` | Equal | User-selected via UI dropdown |
| Equality | `neq` | Not equal | User-selected via UI dropdown |
| Comparison | `lt` | Less than | `numeric` only |
| Comparison | `lte` | Less than or equal | `numeric` only |
| Comparison | `gt` | Greater than | `numeric` only |
| Comparison | `gte` | Greater than or equal | `numeric` only |
| String | `prefix` | Starts with | `text` only (auto-coercion) |
| String | `suffix` | Ends with | `text` only (auto-coercion) |
| Existence | `is_null` | Field is null | Any type, `value` ignored |
| Existence | `exists` | Field exists (non-null) | Any type, `value` ignored |

**Operator selection rules**:

- **Comparison operators** (`gt`, `gte`, `lt`, `lte`) always require `field_type = "numeric"` with strict type checking.
- **String operators** (`prefix`, `suffix`) always require `field_type = "text"` with automatic string coercion.
- **Equality operators** (`eq`, `neq`) require explicit UI selection of `field_type` with dropdown and "strict type matching" checkbox. No default provided; forces user awareness.
- **Existence operators** (`is_null`, `exists`) work with any `field_type` and ignore `value` field.

**Why no regex**: Regular expressions too slow for high-throughput evaluation. String prefix/suffix operations optimize to simple byte comparisons. Future consideration: bloom filters for set membership.

### 5. Field Type Behavior Summary

Brief summary; see ADR-016 for complete type coercion rules:

| field_type | Used With | Coercion Behavior |
|------------|-----------|-------------------|
| `numeric` | Comparison operators | Strict: `"25"` → 25 ✓, `"abc"` → error |
| `text` | String operators | Auto-coercion: `100` → `"100"` ✓ |
| `boolean` | Equality operators | Strict: `true` ✓, `"true"` → error |
| `any` | Equality operators | Lenient: `"25" == 25` ✓ (coerced) |

**Null handling**: Null values always treated as missing field, defers to `on_missing_field` configuration (default `"skip"`).

### 6. Priority Calculation

Priority determines rule evaluation order (lower priority = earlier evaluation):

**Calculation algorithm** (pseudocode):
```
base_priority = 1000

# Count complexity factors
condition_count = sum(len(group['all']) for group in rule['any'])
or_penalty = len(rule['any']) * 10

# Operator costs (cheaper operators = lower priority)
cost_map = {
  'exists': 1,
  'is_null': 1,
  'eq': 5,
  'neq': 5,
  'lt': 7,
  'lte': 7,
  'gt': 7,
  'gte': 7,
  'prefix': 10,
  'suffix': 10
}

operator_cost = sum(cost_map[cond['op']] for all conditions)

# Sample rate adjustment (higher sampling = higher priority)
sample_penalty = int((1.0 - rule['sample_rate']) * 50)

calculated_priority = base_priority + condition_count + or_penalty + operator_cost + sample_penalty
```

**Rationale**:
- Simpler rules evaluate first (fewer conditions = lower latency).
- Existence checks cheapest (no value comparison).
- String operations more expensive than numeric comparison.
- Low sample-rate rules deferred (skip most records anyway).
- Dynamic calculation eliminates manual priority management and concurrent modification conflicts.

**API behavior**: Priority calculated on every rule fetch. Sensors receive rules sorted by priority. Client SDK evaluates in order, stops on first match.

### 7. Performance Optimizations

**Short-circuit evaluation**:
- Within `all` group: Stop on first false condition
- Across `any` groups: Stop on first matching group
- Firewall-style first-match-wins across rules

**Cost-based predicate ordering** (within each `all` group):
1. Existence checks (`exists`, `is_null`) first
2. Equality checks (`eq`, `neq`) second
3. Numeric comparisons (`lt`, `lte`, `gt`, `gte`) third
4. String operations (`prefix`, `suffix`) last

**Sample rate fast paths**:
- `sample_rate == 1.0`: Skip random number generation entirely
- `sample_rate == 0.0`: Skip rule immediately (return no match)
- Apply sampling before field extraction to avoid wasted work

**Future optimizations** (deferred from MVP):
- Range merging for same-field comparisons (e.g., `temp > 10 AND temp < 20` → single range check)
- Zero-allocation field lookups using direct memory access or buffer views
- Branchless comparison operations using SIMD instructions
- Bloom filters for high-cardinality set membership (`value IN [...]`)

**Rationale**: MVP optimizations sufficient for <1ms target. Measurements inform future work.

### 8. UI Design Implications

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
- Field selector: Dropdown or autocomplete from schema hints (see ADR-015)
- Operator dropdown: Available operators depend on selected field type
- Field type selector: Auto-selected for comparison/string operators, explicit dropdown for equality
- Value input: Type-appropriate widget (number input, text input, checkbox)

**No JSON editing required**: All rule construction through visual components. Advanced mode may expose JSON for power users, but not required for basic usage.

**Rationale**: DNF structure prevents users from creating unparseable boolean expressions. Visual builder eliminates syntax errors. Direct correspondence between UI and schema simplifies implementation.

### 9. Wire Protocol vs Documentation Format

**Critical distinction**:
- **This ADR uses JSON** for human readability
- **Implementation uses Protocol Buffers** for performance and type safety
- JSON examples represent logical schema, not actual wire format

**Protobuf benefits**:
- Compile-time type checking prevents schema errors
- Binary encoding reduces bandwidth for rule sync
- Code generation ensures type-safe SDK implementations
- Schema evolution with backward compatibility guarantees

**Implementation note**: Complete protobuf definitions in ADR-004 (API Service Architecture). Rule schema fields map directly to protobuf message types.

## Consequences

### Benefits

1. **Performance**: Pre-compiled predicates enable <1ms evaluation target. Short-circuit evaluation minimizes wasted work.
2. **Usability**: DNF structure maps naturally to visual builder. Non-programmers can construct complex rules without JSON.
3. **Predictability**: Explicit OR/AND semantics eliminate ambiguity. First-match firewall behavior matches user expectations.
4. **Type Safety**: Per-condition `field_type` catches type errors at rule evaluation, not data ingestion.
5. **UI Simplicity**: Direct correspondence between schema and visual components reduces implementation complexity.
6. **Dynamic Priority**: Automatic priority calculation eliminates manual management and concurrent modification conflicts.
7. **Sampling Efficiency**: Early sampling skips expensive field extraction for records that won't be evaluated.
8. **Schema Evolution**: `on_missing_field` configuration enables graceful handling of schema changes (see ADR-016).

### Tradeoffs

1. **Limited Expressiveness**: DNF restricts boolean logic compared to arbitrary expression trees. Cannot express `NOT (A OR B)` without De Morgan transformation.
2. **No Regex**: String matching limited to prefix/suffix. Cannot validate email formats, parse structured strings, or match complex patterns.
3. **Manual De Morgan**: Users must manually convert `NOT (A AND B)` to `(NOT A) OR (NOT B)` using `neq` operators.
4. **Field Type Annotation Overhead**: Every condition requires explicit `field_type`, increasing schema verbosity.
5. **No Cross-Field Comparisons**: Cannot express `temperature > high_threshold` where both are record fields. Values must be literals.
6. **Priority Opacity**: Dynamically calculated priority invisible to users. Unexpected evaluation order if calculation changes.
7. **Wildcard Limitations**: ANY semantics only (see ADR-015). Cannot express "ALL elements must satisfy" without multiple conditions.

### Operational Implications

1. **Rule Debugging**: DNF structure enables clear explanation of why rule matched (which group, which conditions).
2. **Performance Monitoring**: Log per-rule evaluation time to identify expensive rules. Use for priority tuning.
3. **Schema Hints**: API should provide field path suggestions based on recently seen events to guide UI autocomplete.
4. **Version Migration**: Schema version field enables future rule format changes without breaking existing rules.
5. **A/B Testing**: Dry-run mode (see ADR-017) enables testing new rules against production traffic before activation.

## Implementation

1. **Define protobuf schema** for rule message:
   - Map JSON fields to protobuf types
   - Use `repeated` for arrays (`any`, `all`)
   - Enum types for `action`, `on_missing_field`, `op`, `field_type`
   - Use `google.protobuf.Value` for condition `value` (polymorphic type)

2. **Implement rule parser** in each SDK:
   - Parse protobuf rule message
   - Compile to native predicate functions that accept a Record and return a boolean result
   - Apply cost-based ordering within each `all` group
   - Pre-bind field accessors during compilation

3. **Implement evaluation engine**:
   - Iterate through `any` groups in order
   - For each group, evaluate `all` conditions with short-circuit
   - Stop on first matching group
   - Return matched field path and value for event reporting
   - Handle missing fields per `on_missing_field` policy (see ADR-016)

4. **Implement priority calculation** in API server:
   - Calculate priority on rule creation/update
   - Store in memory only (not persisted)
   - Return rules to sensors sorted by priority
   - Log priority values for debugging

5. **Implement sampling**:
   - Check `sample_rate` before field extraction
   - Fast path for 0.0 and 1.0 (no RNG)
   - Use cryptographically secure RNG for fairness
   - Deterministic sampling seed for reproducibility (optional)

6. **Implement Web UI rule builder**:
   - React component for DNF structure (tabs for OR, lists for AND)
   - Field path autocomplete from schema hints API
   - Operator dropdown filtered by selected field type
   - Value input validation based on field type
   - Real-time preview of generated rule JSON (advanced mode)

7. **Add validation layer**:
   - Reject rules with empty `any` or `all` arrays
   - Validate operator/field_type combinations
   - Enforce name/description length limits
   - Validate UUIDv7 format for rule_id
   - Check tag format per ADR-001

## Related Decisions

**Depends on:**
- **ADR-001: Architectural Principles** - Implements Schema-Agnostic Architecture through runtime field resolution

**Extended by:**
- **ADR-015: Field Path Resolution** - Defines the field path notation and wildcard semantics
- **ADR-016: Type System and Coercion** - Specifies type handling for rule evaluation
- **ADR-018: Rule Lifecycle** - Adds operational controls for rules

## Future Considerations

- **Complex operators**: `IN` for set membership, `BETWEEN` for range checks, `MATCHES` for glob patterns (not regex)
- **Cross-field comparisons**: `field_a > field_b` syntax for dynamic thresholds
- **Computed fields**: `length(name) > 10`, `upper(status) == "ACTIVE"` with limited function set
- **Negation groups**: Explicit `NOT` operator to avoid manual De Morgan transformations
- **Rule templates**: Pre-built rules for common patterns (PII detection, range validation, null checking)
- **Visual debugger**: Step-through evaluation showing which conditions matched/failed
- **Performance profiler**: Per-condition timing to identify expensive operations
- **Schema inference**: Auto-suggest field paths and types from recent events
- **Rule composition**: Reference other rules by ID to enable rule reuse (`RULE(id) AND condition`)
- **Temporal operators**: `CHANGED(field)`, `RATE(field, window)` for stateful evaluation (requires major architecture change)

## Appendix A: Example Rules

### Example 1: Simple Numeric Range
```json
{
  "name": "Temperature out of range",
  "action": "drop",
  "any": [
    {
      "all": [
        {"field": ["temperature"], "field_type": "numeric", "op": "lt", "value": -40}
      ]
    },
    {
      "all": [
        {"field": ["temperature"], "field_type": "numeric", "op": "gt", "value": 150}
      ]
    }
  ]
}
```
Matches if `temperature < -40 OR temperature > 150`.

### Example 2: Complex AND with Multiple Fields
```json
{
  "name": "High-value PII transaction",
  "action": "observe",
  "any": [
    {
      "all": [
        {"field": ["amount"], "field_type": "numeric", "op": "gt", "value": 10000},
        {"field": ["customer", "ssn"], "field_type": "text", "op": "exists", "value": null},
        {"field": ["region"], "field_type": "text", "op": "eq", "value": "US"}
      ]
    }
  ]
}
```
Matches if `amount > 10000 AND customer.ssn exists AND region == "US"`.

### Example 3: Wildcard Array Matching
```json
{
  "name": "Any sensor reading over threshold",
  "action": "error",
  "any": [
    {
      "all": [
        {"field": ["sensors", "*", "value"], "field_type": "numeric", "op": "gt", "value": 100}
      ]
    }
  ]
}
```
Matches if **any** element in `sensors` array has `value > 100` (see ADR-015 for wildcard semantics).

### Example 4: String Prefix with Sampling
```json
{
  "name": "Debug API calls",
  "action": "observe",
  "sample_rate": 0.01,
  "any": [
    {
      "all": [
        {"field": ["api_endpoint"], "field_type": "text", "op": "prefix", "value": "/debug/"}
      ]
    }
  ]
}
```
Matches 1% of records where `api_endpoint` starts with `/debug/`.
