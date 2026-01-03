---
doc_type: spoke
status: active
primary_category: performance
hub_document: doc/05-performance/README.md
tags:
  - sampling
  - optimization
  - performance
---

# Sampling and Performance Optimization

## Context

TrapperKeeper processes high-volume data streams where customers generate millions to billions of records per day. Probabilistic sampling enables flexible performance control through per-rule configuration, allowing expensive rules to coexist with high-throughput pipelines without creating bottlenecks.

**Hub Document**: This document is part of the Performance Architecture. See [Performance Hub](README.md) for strategic overview and cost model details.

## Random Sampling Before Rule Evaluation

Sampling provides two-level performance strategy: probabilistic sampling before rule evaluation, and deterministic optimizations during evaluation.

### Sample Rate Configuration

Each rule has an optional `sample_rate` field (float, range 0.0-1.0, default 1.0):

```json
{
  "rule_id": "...",
  "name": "Sample 10% of records for debugging",
  "sample_rate": 0.1,
  "action": "observe",
  "any": [...]
}
```

**Implementation**:

- Sampling decision made **before rule evaluation begins** (before field extraction, before condition checks)
- Random number generated once per (record, rule) pair: `random() < sample_rate`
- Special-case optimization: Skip RNG entirely for `sample_rate = 0.0` (always skip) and `sample_rate = 1.0` (always evaluate)
- Sample decisions are independent per rule (different rules can sample same record differently)

**Critical clarification**: Sampling is applied to records/rows BEFORE evaluating the rule conditions. With a 1% sample rate, only 1% of records are evaluated against the rule conditions. This is NOT post-filter sampling where all records are evaluated and then 1% of matches are kept.

### Implementation Approaches

**Option A - Explicit sampling first** (typically faster):

```python
sampled_rows = df.sample(frac=sample_rate)
results = sampled_rows[sampled_rows['temp'] > 100]
```

**Option C - Vectorized boolean mask with per-row probability**:

```python
sample_mask = np.random.random(len(df)) < sample_rate
results = df[sample_mask & (df['temp'] > 100)]
```

**Choose the fastest implementation**: Option A typically performs better because it avoids computing the condition mask (`df['temp'] > 100`) for records not in the sample. The purpose of sampling is to reduce performance impact, so choose whichever approach is fastest for the specific framework.

**Rationale**: Sampling before rule evaluation maximizes performance benefit by skipping expensive field extraction and condition evaluation entirely. Per-rule sampling allows mixing high-sampling debug rules with low-sampling production rules. This approach maintains consistency with the performance-first design principle.

## Cost-Based Predicate Ordering

Within each rule's condition groups, conditions are reordered by estimated cost before execution.

### Cost Model

- **Total cost** = Field lookup cost + Operator evaluation cost
- **Field lookup cost**: 128 per string component, 0 for wildcards/indices
- **Operator evaluation cost**: Base operator cost × Field type multiplier × Execution multiplier (8^n for n wildcards)

### Cost Ranking (Cheapest to Most Expensive)

1. **Existence checks**: `exists`, `is_null` (minimal operator cost: 1)
2. **Equality with primitives**: `eq`, `neq` with boolean/numeric fields (operator cost: 5, type multiplier: 1)
3. **Numeric comparisons**: `lt`, `lte`, `gt`, `gte` (operator cost: 7, type multiplier: 1)
4. **Equality with strings**: `eq`, `neq` with text fields (operator cost: 5, type multiplier: 48)
5. **String prefix/suffix**: `prefix`, `suffix` (operator cost: 10, type multiplier: 48)
6. **Single wildcard evaluation**: Condition with one `[*]` in field path (execution multiplier: 8)
7. **Nested wildcard evaluation**: Conditions with multiple `[*]` in field path (execution multiplier: 8^n where n = wildcard count)
   - Double nesting (`a[*].b[*]`): execution multiplier 64
   - Triple+ nesting: **Rejected by API** (see nested wildcard limits)
8. **Deeply nested fields**: Conditions accessing fields >3 levels deep (higher lookup cost: 128 × depth)

### Example Transformation

**Original rule**:

```json
{
  "all": [
    {
      "field": ["readings", "*", "temp"],
      "op": "gt",
      "value": 100,
      "field_type": "int"
    },
    { "field": ["sensor_id"], "op": "exists" },
    {
      "field": ["device_name"],
      "op": "prefix",
      "value": "PROD-",
      "field_type": "string"
    }
  ]
}
```

**Cost calculation**:

1. `sensor_id` exists: 128 (lookup) + 1 (operator) = 129
2. `readings[*].temp > 100`: 384 (lookup) + 56 (7 × 1 × 8) = 440
3. `device_name` prefix: 128 (lookup) + 480 (10 × 48 × 1) = 608

**Optimized execution order**:

1. `sensor_id` exists (cost: 129)
2. `readings[*].temp > 100` (cost: 440)
3. `device_name` starts with "PROD-" (cost: 608)

**Rationale**: Short-circuit evaluation stops at first failed condition. Checking cheap conditions first increases probability of early exit, reducing average evaluation time. The two-part cost model (field lookup + operator evaluation) aligns with rule priority calculation and reflects actual CPU performance characteristics.

**Cross-References**:

- Cost Model: Complete calculation algorithm
- Rule Expression Language: Priority calculation

## Short-Circuit Evaluation (Performance Optimization)

**CRITICAL**: Short-circuit behavior is a **performance optimization, not a semantic requirement**. SDK implementors have discretion on whether and how to implement it.

### Sequential Rule Evaluation

- Rules evaluated in priority order
- Stop at first matching rule (firewall-style)
- Later rules never evaluated if earlier rule matches

### Within-Rule Short-Circuit (Optional Optimization)

- `any` groups: May stop at first matching group (OR semantics)
- `all` conditions: May stop at first failed condition (AND semantics)
- Wildcard arrays: May stop at first matching element (ANY semantics)

### Scope Clarification

- **Per-condition**: Short-circuit applies within a single condition (e.g., evaluating elements in a wildcard array)
- **Per-rule**: Short-circuit applies across conditions within a rule (e.g., stop after first failed condition in `all` group)
- **NOT per-batch**: Batch processing evaluates all rows; short-circuit does not abandon entire batch on first match

### Example

```json
{
  "any": [
    {"all": [condition_A, condition_B]},  // Group 1
    {"all": [condition_C, condition_D]}   // Group 2
  ]
}
```

If Group 1's `condition_A` fails, implementation may skip `condition_B` and evaluate Group 2.
If Group 2's `condition_C` succeeds and `condition_D` succeeds, stop (rule matched).

### Vectorized Operations

For frameworks like Pandas/Spark, short-circuit may apply per-condition but not per-row:

- All rows in batch evaluated (no per-batch short-circuit)
- Within each condition, implementation may short-circuit (e.g., stop checking array elements after first match)
- This allows vectorized operations to maintain performance while optionally optimizing condition evaluation

**Rationale**: Short-circuit evaluation is a standard compiler optimization that reduces unnecessary work when feasible. Combined with cost-based ordering, this provides significant performance gains for rules with multiple conditions. SDK implementors choose appropriate strategy for their execution model (iterative vs vectorized).

## Sampling Implementation Pseudo-code

### Single-Record Evaluation

```python
# Pseudo-code for SDK implementation
def evaluate_rule(record, rule):
    # Special-case optimization
    if rule.sample_rate == 0.0:
        return None  # Skip immediately

    if rule.sample_rate == 1.0:
        pass  # Skip RNG, always evaluate
    else:
        # Random sampling check BEFORE any evaluation
        if random.random() >= rule.sample_rate:
            return None  # Not sampled - skip field extraction and evaluation

    # Only if sampled: continue with field extraction and evaluation
    return evaluate_conditions(record, rule)
```

### Batch/Vectorized Evaluation

```python
# Option A: Explicit sampling first (typically faster)
def evaluate_rule_batch_option_a(df, rule):
    if rule.sample_rate == 0.0:
        return []

    if rule.sample_rate == 1.0:
        sampled_df = df
    else:
        # Sample rows BEFORE evaluation
        sampled_df = df.sample(frac=rule.sample_rate)

    # Evaluate only sampled subset
    matches = sampled_df[sampled_df['temp'] > 100]
    return matches

# Option C: Vectorized boolean mask (alternative)
def evaluate_rule_batch_option_c(df, rule):
    if rule.sample_rate == 0.0:
        return []

    if rule.sample_rate == 1.0:
        sample_mask = np.ones(len(df), dtype=bool)
    else:
        # Per-row probability mask
        sample_mask = np.random.random(len(df)) < rule.sample_rate

    # Combined mask: sampled AND condition
    matches = df[sample_mask & (df['temp'] > 100)]
    return matches

# Choose Option A unless profiling shows Option C is faster
```

## Short-Circuit Evaluation Flow

```python
# Pseudo-code for rule evaluation with optional short-circuit
def evaluate_rule(record, rule):
    # Check sampling first
    if not should_sample(rule.sample_rate):
        return None

    # Evaluate OR groups (may short-circuit on first match)
    for group in rule.any:
        # Evaluate AND conditions (may short-circuit on first failure)
        all_match = True
        for condition in sorted_by_cost(group.all):
            if not evaluate_condition(record, condition):
                all_match = False
                break  # Optional short-circuit: SDK may skip remaining conditions

        if all_match:
            return create_match_event(record, rule, group)

    return None  # No groups matched

# Note: Vectorized implementations (Pandas/Spark) may evaluate all conditions
# for all rows, trading per-condition short-circuit for vectorization benefits.
# Per-batch short-circuit does NOT apply: all rows in batch are evaluated.
```

## Performance Characteristics

**Expected latencies** (per record, local evaluation only):

| Scenario                                 | Latency Target                             |
| ---------------------------------------- | ------------------------------------------ |
| Sample rate = 0.0                        | ~10 nanoseconds (RNG skip + early return)  |
| Sample rate < 1.0, not sampled           | ~100 nanoseconds (RNG call + early return) |
| Simple rule (3 conditions, no wildcards) | ~100-500 microseconds                      |
| Complex rule (10+ conditions, wildcards) | ~500 microseconds - 2ms                    |
| Batch of 1000 records, simple rules      | <100ms (100μs/record average)              |

**Bottleneck identification**:

- If >50% of time in field extraction: Consider caching intermediate field lookups (post-MVP)
- If >50% of time in type coercion: Review rule `field_type` configurations for unnecessary coercions
- If >50% of time in wildcard evaluation: Consider restructuring data to reduce array traversals

**Cross-References**:

- Optimization Strategies: Systematic bottleneck analysis and mitigation strategies

## Monitoring and Profiling

Add instrumentation to track:

- Average evaluation time per rule (p50, p95, p99)
- Conditions evaluated per record (indicates short-circuit effectiveness)
- Sample rate effectiveness (% of records skipped)
- Field extraction cache hit rate (post-MVP optimization)

Export metrics via operational endpoints.

**Cross-References**:

- Operational Endpoints: Health check and metrics exposure

## Edge Cases and Limitations

**Known Limitations**:

- **Non-deterministic sampling**: Random sampling means some records skip rules unpredictably (acceptable for observability use case)
- **No exact percentages**: Over small batches, actual sample rate varies statistically around target
- **Cost heuristics accuracy**: Cost ranking is approximate; actual performance varies by data shape
- **Reordering limits**: Can only reorder conditions within `all` groups (cannot reorder across `any` groups due to semantics)

**Edge Cases**:

- **Sample rate 0.0**: RNG bypassed entirely, immediate return (zero-cost abstraction)
- **Sample rate 1.0**: RNG bypassed entirely, always evaluate (zero-cost abstraction)
- **Empty rule set**: No conditions to evaluate, no sampling overhead
- **Single condition**: Cost-based ordering has no effect, sampling still applies

**Cross-References**:

- Rule Expression Language: Rule semantics and evaluation order constraints

## Related Documents

**Dependencies** (read these first):

- Rule Expression Language: Extends rules with sample_rate field for probabilistic evaluation
- Field Path Resolution: Optimizes field extraction and wildcard evaluation
- Cost Model: Canonical cost calculation for condition ordering

**Related Spokes** (siblings in this hub):

- Optimization Strategies: Trade-off decision framework using sampling
- Batch Processing: Vectorized evaluation complements sampling strategies

**Extended by** (documents building on this):

- SDK Model: Pre-compilation architecture enables fast rule evaluation optimized by sampling
