# ADR-015: Field Path Resolution and Wildcard Semantics

## Revision log

| Date | Description |
|------|-------------|
| 2025-10-28 | Document created |

## Context

TrapperKeeper operates on arbitrary data structures without schema pre-registration. Rules must reference fields in nested objects and arrays using a consistent, predictable notation. Key design constraints:

- **Schema-agnostic architecture**: Server has no knowledge of data schemas (see ADR-023)
- **Nested data support**: JSON-like structures with arbitrary nesting depth
- **Array operations**: Rules must evaluate conditions against array elements
- **Framework diversity**: Field paths must work identically across Python (Pandas), Java (Spark), and Go
- **Performance requirements**: Path resolution must complete in microseconds, not milliseconds
- **Empty array handling**: Clear semantics for conditions that reference non-existent elements
- **Type coercion interaction**: Field resolution must work with type coercion (see ADR-016)

Ambiguities to resolve:
- How should wildcards behave? Match ANY element or ALL elements?
- What happens when referencing empty arrays?
- Should evaluation short-circuit on first match (firewall-style)?
- How are matched_field and matched_value resolved with wildcards?

## Decision

We will implement a **simplified dot/bracket notation** with **ANY-semantics wildcards** and **first-match short-circuit evaluation**.

### 1. Field Path Notation

Rules reference fields using dot/bracket syntax, transmitted as JSON arrays:

| User Input Syntax | JSON Path Array | Meaning |
|-------------------|-----------------|---------|
| `customer.address.zipcode` | `["customer", "address", "zipcode"]` | Nested objects |
| `sensors[3].value` | `["sensors", 3, "value"]` | Array index (zero-based) |
| `readings[*].temp` | `["readings", "*", "temp"]` | Wildcard (ANY semantics) |
| `data["field.with.dots"]` | `["data", "field.with.dots"]` | Escaped keys for fields containing dots |

**Rationale**:
- **Array representation**: JSON arrays are unambiguous and language-agnostic (no parsing ambiguity)
- **Dot notation in UI**: Familiar syntax for rule authors, converted to array by UI
- **Bracket escaping**: Handles edge case of field names containing dots (e.g., DNS records, metric names)
- **Zero-based indexing**: Aligns with Python, Java, JavaScript conventions

### 2. Wildcard Semantics (ANY, not ALL)

Wildcards use **ANY semantics**: A condition matches if **any element** satisfies it.

**Example**:
```json
{
  "readings": [
    {"temp": 10},
    {"temp": 30},
    {"temp": 50}
  ]
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

### 3. Empty Array Behavior (No Match)

Empty arrays result in **no match** (least intrusive by default):

**Example**:
```json
{"readings": []}
```

Rule: `readings[*].temp > 15` → **Does not match**

**Rationale**:
- **Consistent with ANY semantics**: If no elements exist, there's nothing to match
- **Least intrusive principle**: Pass-through by default (see ADR-023)
- **Prevents false positives**: Empty arrays should not trigger drop/error actions
- **Clear semantics**: "Match if any element satisfies condition" → no elements = no match

### 4. First-Match Short-Circuit Evaluation

Wildcard evaluation **stops at the first matching element**:

**Example**:
```json
{
  "readings": [
    {"temp": 10},
    {"temp": 30},
    {"temp": 50}
  ]
}
```

Rule: `readings[*].temp > 15` → Evaluates readings[0] (10 > 15 = false), then readings[1] (30 > 15 = **true**), **stops** without checking readings[2]

**Rationale**:
- **Performance**: Avoids unnecessary checks once match found (firewall-style)
- **Aligns with first-match-wins rule execution**: Stop on first matching rule (see ADR-014)
- **Deterministic**: Always returns the earliest matching element, not arbitrary element

### 5. Matched Field and Value Resolution

When a wildcard matches, the **resolved index** and **actual value** are recorded:

**Example**:
```json
{
  "readings": [
    {"temp": 10},
    {"temp": 30},
    {"temp": 50}
  ]
}
```

Rule: `readings[*].temp > 15`
- `matched_field` = `["readings", 1, "temp"]` (wildcard resolved to index 1)
- `matched_value` = `30` (actual value that triggered match)

**Rationale**:
- **Auditability**: Events show exactly which array element caused the match
- **Debugging**: Operators can identify problematic records in large arrays
- **Wildcard resolution**: Replace `*` with actual index for precise field path

### 6. Type Coercion Integration

Field resolution interacts with type coercion (see ADR-016) during wildcard evaluation:

**Null-like values**: Treated as missing field, deferred to `on_missing_field` setting:
```json
{
  "readings": [
    {"temp": null},
    {"temp": 30}
  ]
}
```

Rule: `readings[*].temp > 15` with `on_missing_field="skip"`
- Evaluates readings[0].temp (null → treated as missing → skip to next element)
- Evaluates readings[1].temp (30 > 15 = **true**, match)

**Impossible coercion**: Treat as condition failed, continue to next element:
```json
{
  "readings": [
    {"temp": 10},
    {"temp": "invalid"},
    {"temp": 30}
  ]
}
```

Rule: `readings[*].temp > 15` with `field_type="numeric"`
- Evaluates readings[0].temp (10 > 15 = false)
- Evaluates readings[1].temp ("invalid" cannot coerce to numeric → condition failed)
- Evaluates readings[2].temp (30 > 15 = **true**, match)
- `matched_field` = `["readings", 2, "temp"]` (skipped index 1)
- `matched_value` = `30`

**Rationale**:
- **Fail-safe wildcard evaluation**: Type errors don't halt entire array scan
- **Continue on coercion failure**: Check remaining elements rather than erroring immediately
- **Defer null handling**: Leverage existing `on_missing_field` logic rather than special-casing

### 7. Nested Wildcards (Out of Scope for MVP)

Nested wildcards (e.g., `departments[*].employees[*].salary`) are **not supported** in MVP:

**Rationale**:
- **Complexity**: Nested iteration requires cartesian product semantics
- **Performance concerns**: N×M iteration could be expensive
- **Unclear semantics**: Should `departments[*].employees[*].salary > 100000` match if ANY department has ANY employee with high salary, or if ALL departments have SOME employee?
- **Deferred to future**: Can be added later with explicit semantics once use cases clarified

### 8. Maximum Path Depth (No Limit)

No enforced limit on field path depth:

**Example**: `["level1", "level2", "level3", "level4", "level5", "level6"]` is valid

**Rationale**:
- **YAGNI principle**: No evidence of pathological cases in customer data
- **Simple implementation**: Easier to add limits later than remove them
- **Framework-dependent limits**: Pandas/Spark may have their own nesting limits that provide natural boundaries

## Consequences

### Benefits

1. **Intuitive Semantics**: ANY-based wildcards match common use cases ("flag if any temperature is high")
2. **Performance**: Short-circuit evaluation minimizes wasted computation
3. **Auditability**: Resolved matched_field shows exactly which element triggered match
4. **Type Safety Integration**: Coercion failures handled gracefully during wildcard scan
5. **Predictable Behavior**: Empty arrays consistently result in no match
6. **Framework Portability**: Array representation works identically in Python, Java, Go
7. **Escaping Support**: Bracket notation handles edge cases like field names with dots

### Tradeoffs

1. **No ALL Semantics**: Cannot express "match if all elements satisfy condition" (workaround: invert logic with neq/lt)
2. **No Nested Wildcards**: Complex nested structures require multiple rules
3. **No Slicing**: Cannot reference ranges like `readings[0:5].temp` (MVP limitation)
4. **Performance Variability**: Large arrays with late matches scan many elements before short-circuiting
5. **Type Error Skipping**: Coercion failures in arrays silently continue (may miss data quality issues)

### Operational Implications

1. **Rule Authoring**: UI must convert dot notation to array representation before sending to API
2. **Event Storage**: matched_field always contains resolved path (wildcards replaced with indices)
3. **Debugging**: Operators can trace exact array element that caused match
4. **Framework Adapters**: Each SDK must implement field accessor with identical wildcard semantics
5. **Documentation**: Must clearly explain ANY vs ALL semantics to prevent user confusion

## Implementation

1. **SDK Field Accessor**:
   - Implement path resolution function: `resolveField(record, path) → (value, resolvedPath, error)`
   - Handle wildcards with ANY semantics and short-circuit evaluation
   - Return resolved path (replace `*` with matched index)
   - Integrate with type coercion (see ADR-016)

2. **Wildcard Evaluation**:
   - For each array element: attempt field extraction and condition evaluation
   - On null/missing: defer to `on_missing_field` setting, continue to next element
   - On coercion failure: log warning, continue to next element
   - On match: record resolved index and value, return immediately (short-circuit)
   - If no elements match: return no-match

3. **UI Field Path Builder**:
   - Accept dot notation input: `customer.address.zipcode`
   - Convert to JSON array: `["customer", "address", "zipcode"]`
   - Support bracket notation for escaping: `data["field.with.dots"]`
   - Provide wildcard selector for array fields: `readings[*].temp`

4. **Event Recording**:
   - Store matched_field with resolved indices (no wildcards)
   - Store matched_value as extracted value
   - Include both in event schema (see ADR-013)

5. **Testing**:
   - Unit tests for nested paths, array indexing, wildcards
   - Test empty array behavior (must not match)
   - Test type coercion interaction (null, impossible coercion)
   - Test short-circuit behavior (verify early termination)
   - Test escaped field names with dots

6. **Documentation**:
   - Provide examples of common field path patterns
   - Clarify ANY vs ALL semantics with visual examples
   - Document bracket escaping for field names with special characters
   - Explain performance implications of wildcard position (early vs late match)

## Related Decisions

**Depends on:**
- **ADR-014: Rule Expression Language** - Extends the rule language with detailed field path resolution semantics

**Related to:**
- **ADR-016: Type System and Coercion** - Field resolution results are processed through type coercion
- **ADR-017: Schema Evolution** - Handles missing fields during path resolution

## Future Considerations

- **Nested wildcards**: Support `departments[*].employees[*].salary` with clear semantics
- **Array slicing**: Enable `readings[0:5].temp` to reference ranges
- **ALL semantics**: Add `all()` function for "match if all elements satisfy condition"
- **Performance optimization**: Index-based fast paths for non-wildcard paths
- **JSONPath compatibility**: Consider adopting standard JSONPath syntax for advanced use cases
- **Compiled field accessors**: Pre-compile field paths to native functions for faster evaluation
- **Max depth enforcement**: Add configurable limit if pathological cases emerge

## Appendix: Common Field Path Patterns

### Simple Nested Object
```
Input: order.customer.email
Array: ["order", "customer", "email"]
```

### Array with Index
```
Input: sensors[0].value
Array: ["sensors", 0, "value"]
```

### Array with Wildcard
```
Input: readings[*].temperature
Array: ["readings", "*", "temperature"]
Semantics: Match if ANY reading has temperature that satisfies condition
```

### Escaped Field Name
```
Input: metrics["response.time.ms"]
Array: ["metrics", "response.time.ms"]
Reason: Field name contains dots, must use bracket notation
```

### Mixed Notation
```
Input: data["system.cpu"].cores[*].utilization
Array: ["data", "system.cpu", "cores", "*", "utilization"]
Semantics: Access field "system.cpu" (escaped), then array of cores with wildcard
```
