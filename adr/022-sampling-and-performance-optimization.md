# ADR-022: Sampling and Performance Optimization

## Revision log

| Date | Description |
|------|-------------|
| 2025-10-28 | Document created |

## Context

TrapperKeeper processes high-volume data streams where customers generate millions to billions of records per day. For industrial IoT customers, this includes 500k-sample waveform bursts arriving every 30 minutes and continuous telemetry streams. For financial customers, this includes high-frequency market data and tick-level trading records.

Key performance requirements:
- **Target latency**: <1ms per record for rule evaluation (excluding network I/O)
- **High throughput**: Support for batch processing frameworks (Pandas, Spark)
- **Resource efficiency**: Minimize CPU and memory overhead on data pipeline hosts
- **Sampling flexibility**: Allow probabilistic evaluation when full inspection isn't required

Performance challenges:
- Field extraction from nested structures has cost
- Complex rules with many conditions increase evaluation time
- Type coercion adds overhead for each comparison
- Wildcard evaluation iterates over array elements
- Rules must execute sequentially (firewall-style first-match-wins)

Design constraints:
- Schema-agnostic architecture means no pre-compilation based on known schemas
- Ephemeral sensors prevent long-running optimizations (e.g., learned query plans)
- MVP simplicity favors straightforward algorithms over complex heuristics
- Must work consistently across all SDKs (Python, Java, Go)

## Decision

We will implement a **two-level performance strategy**: probabilistic sampling before rule evaluation, and deterministic optimizations during evaluation.

### 1. Random Sampling Before Field Extraction

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
- Sampling decision made **before** field extraction begins
- Random number generated once per (record, rule) pair: `random() < sample_rate`
- Special-case optimization: Skip RNG entirely for `sample_rate = 0.0` (always skip) and `sample_rate = 1.0` (always evaluate)
- Sample decisions are independent per rule (different rules can sample same record differently)

**Rationale**: Sampling before field extraction maximizes performance benefit by avoiding expensive operations. Per-rule sampling allows mixing high-sampling debug rules with low-sampling production rules.

### 2. Cost-Based Predicate Ordering

Within each rule's condition groups, conditions are reordered by estimated cost before execution:

**Cost ranking** (cheapest to most expensive):
1. **Existence checks**: `exists`, `is_null` (field lookup only, no value comparison)
2. **Equality with primitives**: `eq`, `neq` with boolean/numeric fields (fast comparison)
3. **Equality with strings**: `eq`, `neq` with text fields (string comparison overhead)
4. **Numeric comparisons**: `lt`, `lte`, `gt`, `gte` (type coercion if needed)
5. **String prefix/suffix**: `prefix`, `suffix` (substring operations)
6. **Wildcard evaluation**: Any condition with `[*]` in field path (array iteration)
7. **Deeply nested fields**: Conditions accessing fields >3 levels deep (traversal overhead)

**Example transformation**:

Original rule:
```json
{
  "all": [
    {"field": ["readings", "*", "temp"], "op": "gt", "value": 100},
    {"field": ["sensor_id"], "op": "exists"},
    {"field": ["device_name"], "op": "prefix", "value": "PROD-"}
  ]
}
```

Optimized execution order:
1. `sensor_id` exists (cheapest)
2. `device_name` starts with "PROD-" (string operation)
3. `readings[*].temp > 100` (most expensive: wildcard + comparison)

**Rationale**: Short-circuit evaluation stops at first failed condition. Checking cheap conditions first increases probability of early exit, reducing average evaluation time.

### 3. Short-Circuit Evaluation

**Sequential rule evaluation**:
- Rules evaluated in priority order
- Stop at first matching rule (firewall-style)
- Later rules never evaluated if earlier rule matches

**Within-rule short-circuit**:
- `any` groups: Stop at first matching group (OR semantics)
- `all` conditions: Stop at first failed condition (AND semantics)
- Wildcard arrays: Stop at first matching element (ANY semantics)

**Example**:
```json
{
  "any": [
    {"all": [condition_A, condition_B]},  // Group 1
    {"all": [condition_C, condition_D]}   // Group 2
  ]
}
```

If Group 1's `condition_A` fails, skip `condition_B` and evaluate Group 2.
If Group 2's `condition_C` succeeds and `condition_D` succeeds, stop (rule matched).

**Rationale**: Short-circuit evaluation is a standard compiler optimization that reduces unnecessary work. Combined with cost-based ordering, this provides significant performance gains for rules with multiple conditions.

### 4. Optimizations Explicitly Deferred

**Out of scope for MVP**:
- **Range merging**: Combining overlapping comparisons on same field (e.g., `x > 10 AND x < 100` → `10 < x < 100`)
- **Zero-allocation field lookups**: Pre-computed field offsets for common paths
- **Branchless comparison operations**: SIMD-style vectorized comparisons
- **Learned query plans**: Statistical profiling of condition selectivity
- **JIT compilation**: Runtime code generation for hot rules
- **Index structures**: Hash tables or tries for field lookups

**Rationale**: These optimizations add significant complexity and require careful benchmarking. MVP focuses on straightforward algorithmic improvements with predictable behavior. Advanced optimizations can be added post-MVP if profiling shows they're needed.

## Consequences

### Benefits

1. **Flexible sampling**: Different rules can sample at different rates, enabling mixed debugging/production workloads
2. **Predictable performance**: Deterministic optimizations work consistently across all data shapes
3. **Tunable resource usage**: Operators can reduce load by decreasing sample rates without changing rule logic
4. **Early-exit optimization**: Cost-based ordering + short-circuit maximizes probability of fast rejection
5. **Zero-cost abstraction**: Sample rate 1.0 has no overhead (RNG skipped)
6. **SDK portability**: Simple algorithms work identically across Python, Java, Go implementations
7. **Debugging support**: Low sample rates enable expensive debug rules without production impact

### Tradeoffs

1. **Non-deterministic sampling**: Random sampling means some records skip rules unpredictably (acceptable for observability use case)
2. **No exact percentages**: Over small batches, actual sample rate varies statistically around target
3. **Cost heuristics accuracy**: Cost ranking is approximate; actual performance varies by data shape
4. **Reordering limits**: Can only reorder conditions within `all` groups (cannot reorder across `any` groups due to semantics)
5. **No cross-rule optimization**: Each rule evaluated independently (can't combine or deduplicate field extractions)
6. **Deferred vectorization**: Row-by-row evaluation leaves performance on table for batch frameworks (see ADR-021)

### Performance Characteristics

**Expected latencies** (per record, local evaluation only):

| Scenario | Latency Target |
|----------|----------------|
| Sample rate = 0.0 | ~10 nanoseconds (RNG skip + early return) |
| Sample rate < 1.0, not sampled | ~100 nanoseconds (RNG call + early return) |
| Simple rule (3 conditions, no wildcards) | ~100-500 microseconds |
| Complex rule (10+ conditions, wildcards) | ~500 microseconds - 2ms |
| Batch of 1000 records, simple rules | <100ms (100μs/record average) |

**Bottleneck identification**:
- If >50% of time in field extraction: Consider caching intermediate field lookups (post-MVP)
- If >50% of time in type coercion: Review rule `field_type` configurations for unnecessary coercions
- If >50% of time in wildcard evaluation: Consider restructuring data to reduce array traversals

## Implementation

### 1. Sampling Implementation

See Appendix A for implementation pseudo-code.

### 2. Cost-Based Ordering

See Appendix B for cost estimation algorithm.

### 3. Short-Circuit Evaluation

See Appendix C for complete evaluation flow with short-circuit logic.

### 4. Monitoring and Profiling

Add instrumentation to track:
- Average evaluation time per rule (p50, p95, p99)
- Conditions evaluated per record (indicates short-circuit effectiveness)
- Sample rate effectiveness (% of records skipped)
- Field extraction cache hit rate (post-MVP optimization)

Export metrics via operational endpoints (see ADR-005).

## Related Decisions

**Depends on:**
- **ADR-014: Rule Expression Language** - Extends rules with sample_rate field for probabilistic evaluation
- **ADR-015: Field Path Resolution** - Optimizes field extraction and wildcard evaluation

**Related to:**
- **ADR-002: SDK Model** - Pre-compilation architecture enables fast rule evaluation optimized by sampling
- **ADR-023: Batch Processing and Vectorization** - Vectorized evaluation for Pandas/Spark complements row-by-row optimizations

## Future Considerations

- **Adaptive sampling**: Automatically adjust sample rates based on observed match frequency
- **Field extraction caching**: Memoize intermediate field lookups within single record evaluation
- **Condition selectivity learning**: Track which conditions most frequently fail, optimize ordering dynamically
- **SIMD vectorization**: Use CPU vector instructions for primitive comparisons in tight loops
- **JIT compilation**: Generate native code for hot rules using LLVM or similar
- **Rule deduplication**: Detect when multiple rules evaluate identical conditions, share results
- **Cost model tuning**: Collect profiling data to refine cost estimates per SDK/platform

## Appendix A: Sampling Implementation Pseudo-code

```python
# Pseudo-code for SDK implementation
def evaluate_rule(record, rule):
    # Special-case optimization
    if rule.sample_rate == 0.0:
        return None  # Skip immediately

    if rule.sample_rate == 1.0:
        pass  # Skip RNG, always evaluate
    else:
        # Random sampling check
        if random.random() >= rule.sample_rate:
            return None  # Not sampled

    # Continue with field extraction and evaluation
    return evaluate_conditions(record, rule)
```

## Appendix B: Cost Estimation Algorithm

```python
# Pseudo-code for condition cost estimation
def estimate_cost(condition):
    cost = 0

    # Operator cost
    if condition.op in ['exists', 'is_null']:
        cost += 1
    elif condition.op in ['eq', 'neq']:
        cost += 2 if condition.field_type == 'text' else 1
    elif condition.op in ['lt', 'lte', 'gt', 'gte']:
        cost += 3
    elif condition.op in ['prefix', 'suffix']:
        cost += 4

    # Wildcard penalty
    if '*' in condition.field:
        cost += 10

    # Nesting depth penalty
    cost += len(condition.field)

    return cost

# Sort conditions by cost before evaluation
sorted_conditions = sorted(conditions, key=estimate_cost)
```

## Appendix C: Short-Circuit Evaluation Flow

```python
# Pseudo-code for rule evaluation with short-circuit
def evaluate_rule(record, rule):
    # Check sampling first
    if not should_sample(rule.sample_rate):
        return None

    # Evaluate OR groups (short-circuit on first match)
    for group in rule.any:
        # Evaluate AND conditions (short-circuit on first failure)
        all_match = True
        for condition in sorted_by_cost(group.all):
            if not evaluate_condition(record, condition):
                all_match = False
                break  # Short-circuit: skip remaining conditions

        if all_match:
            return create_match_event(record, rule, group)

    return None  # No groups matched
```
