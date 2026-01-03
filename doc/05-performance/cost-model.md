---
doc_type: spoke
status: active
primary_category: performance
hub_document: doc/05-performance/README.md
tags:
  - cost-model
  - performance
  - calculation
---

# Cost Model and Calculation Algorithm

## Context

The cost model provides the canonical source of truth for all performance calculations in TrapperKeeper. It defines operator costs, field type multipliers, execution multipliers, and the complete algorithm for calculating rule priority.

**Hub Document**: This document is part of the Performance Architecture. See [Performance Hub](README.md) for strategic overview and optimization guidance.

## Operator Cost Map (Canonical Constants)

Base cost per operator before applying field type or execution multipliers.

### Operator Costs

```python
operator_cost_map = {
    'exists': 1,       # No value comparison needed
    'is_null': 1,      # No value comparison needed
    'eq': 5,           # Simple equality check
    'neq': 5,          # Simple inequality check
    'lt': 7,           # Numeric comparison with ordering
    'lte': 7,          # Numeric comparison with ordering
    'gt': 7,           # Numeric comparison with ordering
    'gte': 7,          # Numeric comparison with ordering
    'in': 8,           # Set membership check
    'prefix': 10,      # String prefix matching
    'suffix': 10       # String suffix matching
}
```

**Rationale**:

- Existence checks cheapest (no value comparison)
- Equality checks simple (single comparison operation)
- Numeric comparisons slightly more expensive (ordering logic)
- String operations most expensive (byte-by-byte comparison)

**IMPORTANT**: These are initial values based on CPU benchmarks. They may be tuned based on production profiling. Changes must be documented with rationale.

Profiling criteria and tuning procedures are deferred to a planned profiling guide (see performance-index.md Known Gaps).

## Field Type Multipliers (Canonical Constants)

Cost adjustment based on field type complexity.

### Type Multipliers

```python
field_type_multipliers = {
    'int': 1,          # Native CPU integer operations (baseline)
    'boolean': 1,      # Single bit comparison (equivalent to int)
    'float': 4,        # Floating-point operations more expensive
    'string': 48,      # String operations involve memory access, loops
    'any': 128         # Type conversion and dynamic dispatch very expensive
}
```

**Rationale**:

- **Integer (1×)**: Baseline - native CPU operations, fastest path
- **Boolean (1×)**: Simple bit comparison, equivalent to integer
- **Float (4×)**: Floating-point unit overhead, precision handling
- **String (48×)**: Memory access patterns, byte-by-byte comparison, length checks
- **Any (128×)**: Dynamic type dispatch, conversion overhead, runtime type checking

**Measurement Basis**: Benchmarked on x86_64 architecture with typical data processing workloads. Your mileage may vary on different architectures (ARM, RISC-V).

**Note**: The `int` and `float` types correspond to the `numeric` field_type in TrapperKeeper's type system, which handles both integer and floating-point values.

**Cross-References**:

- Type System: Complete rationale for field type performance characteristics

## Field Structure Multipliers

Cost adjustment based on field structure complexity (distinct from field type multipliers).

### Structure Multipliers

```python
field_structure_multipliers = {
    'scalar': 1,        # Simple field access (baseline)
    'array': 10,        # Array traversal overhead
    'nested': 10,       # Object nesting traversal
    'array_of_objects': 100  # Combined array + nesting traversal
}
```

**Rationale**:

- **Scalar (1x)**: Baseline - direct field access, no traversal
- **Array (10x)**: Iteration overhead, multiple value checks
- **Nested (10x)**: Nested field resolution, multiple hash lookups
- **Array of Objects (100x)**: Combined cost of array iteration and nested object traversal

**Note**: Field structure multipliers affect lookup costs (traversal complexity), while field type multipliers affect execution costs (operation complexity on the data type).

**Cross-References**:

- Field Path Resolution: How structure affects runtime resolution performance
- Performance Hub: Field structure multiplier rationale

## Field Lookup Cost Formula

Cost of resolving field paths in nested structures.

### Lookup Cost Calculation

```python
FIELD_LOOKUP_COST_PER_COMPONENT = 128

def calculate_field_lookup_cost(field_path):
    """
    Calculate cost of resolving a field path.
    - String component: 128 cost (hash lookup or string comparison)
    - Wildcard (*): 0 cost (iteration handled by execution multiplier)
    - Integer index: 0 cost (direct array access)
    """
    cost = 0
    for component in field_path:
        if isinstance(component, str) and component != '*':
            cost += 128
        # Wildcards and integer indices have zero lookup cost
    return cost
```

### Examples

```python
["temperature"]                                 → 128
["customer", "address", "zipcode"]              → 384  (3 × 128)
["sensors", "*", "value"]                       → 256  (2 × 128, wildcard is free)
["facilities", "*", "sensors", "*", "temp"]     → 384  (3 × 128, wildcards are free)
```

**Rationale**:

- String components require hash table lookups or string comparisons (expensive)
- Wildcards are free at lookup stage (expansion cost captured in execution multiplier)
- Integer indices are direct array access (negligible cost)

## Execution Cost Multiplier (Wildcard Expansion)

Wildcards cause conditions to execute multiple times (once per array element).

### Execution Multiplier Calculation

```python
def calculate_execution_multiplier(field_path):
    """
    Calculate execution cost multiplier based on nested wildcards.
    Each wildcard multiplies execution by 8 (estimated average array size).
    """
    wildcard_count = str(field_path).count('*')
    return 8 ** wildcard_count
```

**Formula**: `8^n` where n = number of nested wildcards

### Examples

```python
["tags"]                  → 8^0 = 1   (no wildcards)
["tags", "*"]             → 8^1 = 8   (single wildcard)
["items", "*", "tags", "*"] → 8^2 = 64  (double nesting)
["a", "*", "b", "*", "c", "*"] → 8^3 = 512  (triple nesting - REJECTED)
```

**Rationale**:

- Assumes average array size of 8 elements (expert guidance as conservative starting point)
- Exponential growth reflects nested iteration (outer loop × inner loop)
- Conservative estimate ensures rules don't underestimate costs
- Can be tuned based on production observations if actual array sizes differ significantly

**Caveat**: Actual array sizes vary. Rules with arrays of 100+ elements will have higher real-world costs than calculated priority suggests.

### Nested Wildcard Validation Limits

**Validation Limits**:

| Nested Wildcards | Execution Multiplier | Cost Range    | Validation Result          | Rationale                 |
| ---------------- | -------------------- | ------------- | -------------------------- | ------------------------- |
| 0                | 1                    | <1,000        | ✅ Allowed                 | No array expansion        |
| 1                | 8                    | 1,000-10,000  | ✅ Allowed                 | Single array iteration    |
| 2                | 64                   | 10,000-50,000 | ✅ Allowed (with sampling) | Nested iteration (max)    |
| 3+               | 512+                 | >50,000       | ❌ REJECTED                | Exponential cost too high |

**Validation Rules**:

- **Hard limit**: Maximum 2 nested wildcards per field path
- **Soft limit**: Maximum 1 nested wildcard without sampling enabled
- **field_ref operator**: ZERO wildcards allowed (must reference exact field)

**Validation Enforcement** (defense in depth):

1. **Web UI**: Makes >2 nested wildcards impossible to create (UI constraint)
2. **API layer**: Returns 400 error for rules with >2 nested wildcards on rule creation
3. **Rule engine**: Prints error for >2 nested wildcards (defense in depth, doesn't reject)
4. **Rule engine**: Prints warning for >1 nested wildcard when `sample_rate` not enabled or = 1.0

**Examples**:

- ✅ `items[*].tags[*]` = 2 wildcards (allowed, enable sampling recommended)
- ❌ `a[*].b[*].c[*]` = 3 wildcards (REJECTED by API with 400 error)
- ❌ `field_ref metadata.items[*].id` (REJECTED - field_ref cannot use wildcards)

**When to Restructure Data**:

If a rule requires >2 nested wildcards, consider these alternatives:

- **Flatten nested arrays** at data ingestion time (denormalize the structure)
- **Pre-aggregate data** to reduce nesting depth before sending to TrapperKeeper
- **Use client-side filtering** to transform data structure before evaluation
- **Split into multiple rules** targeting different nesting levels
- **Reconsider data model** to align with query patterns

**Cross-References**:

- Field Path Resolution: Field path resolution details
- Sampling: Optimization strategies for expensive rules
- Validation Hub: Validation layer specifications

## Complete Cost Calculation Algorithm

The canonical algorithm for calculating rule priority.

### Rule Priority Formula

```python
def calculate_rule_priority(rule):
    """
    Calculate rule priority for evaluation ordering.
    Lower priority = earlier evaluation.
    """
    base_priority = 1000

    # Sum all condition costs across all OR groups
    total_condition_cost = sum(
        calculate_condition_cost(condition)
        for group in rule['any']
        for condition in group['all']
    )

    # OR penalty: more groups = more complex
    or_penalty = len(rule['any']) * 10

    # Sample rate adjustment: lower sampling = lower priority
    sample_penalty = int((1.0 - rule['sample_rate']) * 50)

    return base_priority + total_condition_cost + or_penalty + sample_penalty


def calculate_condition_cost(condition):
    """
    Calculate total cost for a single condition.
    Two-part model: field lookup + operator evaluation.
    """
    # Part 1: Field lookup cost
    field_lookup_cost = calculate_field_lookup_cost(condition['field'])

    # Add field_ref lookup cost if present (cross-field comparison)
    if 'field_ref' in condition:
        field_lookup_cost += calculate_field_lookup_cost(condition['field_ref'])

    # Part 2: Operator evaluation cost
    operator_base_cost = operator_cost_map[condition['op']]
    field_type_mult = field_type_multipliers[condition['field_type']]
    execution_mult = calculate_execution_multiplier(condition['field'])

    operator_evaluation_cost = operator_base_cost * field_type_mult * execution_mult

    # Total cost
    return field_lookup_cost + operator_evaluation_cost
```

### Concrete Examples

**Example 1**: `metadata.tenant.id == "acme"` (string field, no wildcards)

```python
field_lookup_cost = 128 × 3 = 384
operator_evaluation_cost = 5 (eq) × 48 (string) × 1 (no wildcards) = 240
condition_cost = 384 + 240 = 624
```

**Example 2**: `tags[*] prefix "production"` (string field, 1 wildcard)

```python
field_lookup_cost = 128 × 1 = 128  (wildcard is free)
operator_evaluation_cost = 10 (prefix) × 48 (string) × 8 (1 wildcard) = 3,840
condition_cost = 128 + 3,840 = 3,968
```

**Example 3**: `facilities[*].sensors[*].temp > 100` (numeric field, 2 wildcards)

```python
field_lookup_cost = 128 × 3 = 384  (wildcards are free)
operator_evaluation_cost = 7 (gt) × 1 (int) × 64 (2 wildcards) = 448
condition_cost = 384 + 448 = 832
```

**Example 4**: `facilities[*].sensors[*].status starts_with "ALARM-"` (string field, 2 wildcards)

```python
field_lookup_cost = 128 + 0 + 128 + 0 + 128 = 384  (wildcards are free)
operator_evaluation_cost = 10 (prefix) × 48 (string) × 64 (2 wildcards) = 30,720
condition_cost = 384 + 30,720 = 31,104
```

**Analysis**: Extremely expensive (>10,000 guideline). Recommendations:

1. Enable sampling: `sample_rate: 0.1` (10%) reduces effective cost by 90%
2. Consider data restructuring to flatten nested arrays
3. Evaluate if rule can be split or simplified

**Cross-References**:

- Rule Expression Language: Priority calculation integration with rule ordering and tie-breaking

## Reference Implementation

Complete Python reference implementation.

```python
# Performance constants (canonical values)
FIELD_LOOKUP_COST_PER_COMPONENT = 128
BASE_PRIORITY = 1000
OR_GROUP_PENALTY = 10
SAMPLE_RATE_PENALTY_FACTOR = 50

operator_cost_map = {
    'exists': 1,
    'is_null': 1,
    'eq': 5,
    'neq': 5,
    'lt': 7,
    'lte': 7,
    'gt': 7,
    'gte': 7,
    'in': 8,
    'prefix': 10,
    'suffix': 10
}

field_type_multipliers = {
    'int': 1,
    'boolean': 1,
    'float': 4,
    'string': 48,
    'any': 128
}

def calculate_field_lookup_cost(field_path):
    """Calculate cost of resolving a field path."""
    cost = 0
    for component in field_path:
        if isinstance(component, str) and component != '*':
            cost += FIELD_LOOKUP_COST_PER_COMPONENT
    return cost

def calculate_execution_multiplier(field_path):
    """Calculate execution cost multiplier based on nested wildcards."""
    wildcard_count = str(field_path).count('*')
    return 8 ** wildcard_count

def calculate_condition_cost(condition):
    """Calculate total cost for a single condition."""
    # Part 1: Field lookup cost
    field_lookup_cost = calculate_field_lookup_cost(condition['field'])

    # Add field_ref lookup cost if present
    if 'field_ref' in condition:
        field_lookup_cost += calculate_field_lookup_cost(condition['field_ref'])

    # Part 2: Operator evaluation cost
    operator_base_cost = operator_cost_map[condition['op']]
    field_type_mult = field_type_multipliers.get(
        condition['field_type'],
        field_type_multipliers['any']
    )
    execution_mult = calculate_execution_multiplier(condition['field'])

    operator_evaluation_cost = (
        operator_base_cost * field_type_mult * execution_mult
    )

    return field_lookup_cost + operator_evaluation_cost

def calculate_rule_priority(rule):
    """Calculate rule priority for evaluation ordering."""
    # Sum all condition costs
    total_condition_cost = sum(
        calculate_condition_cost(condition)
        for group in rule['any']
        for condition in group['all']
    )

    # OR penalty
    or_penalty = len(rule['any']) * OR_GROUP_PENALTY

    # Sample rate adjustment
    sample_rate = rule.get('sample_rate', 1.0)
    sample_penalty = int((1.0 - sample_rate) * SAMPLE_RATE_PENALTY_FACTOR)

    return BASE_PRIORITY + total_condition_cost + or_penalty + sample_penalty
```

## Related Documents

**Dependencies** (read these first):

- Rule Expression Language: Rule structure and operator semantics

**Related Spokes** (siblings in this hub):

- Optimization Strategies: Trade-off decision framework using these cost calculations
- Sampling: Cost-based condition ordering uses this cost model
- Batch Processing: Vectorized operations and cost implications

**Extended by** (documents building on this):

- Validation Hub: Nested wildcard validation layer specifications
