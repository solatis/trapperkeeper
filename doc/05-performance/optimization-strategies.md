---
doc_type: spoke
status: active
date_created: 2025-11-07
primary_category: performance
hub_document: /Users/lmergen/git/trapperkeeper/doc/05-performance/README.md
tags:
  - optimization
  - performance
  - trade-offs
---

# Optimization Strategies and Trade-off Framework

## Context

Performance optimization requires systematic trade-off analysis balancing sampling, data restructuring, rule splitting, operator substitution, and caching strategies. This document provides a comprehensive decision framework for developers when rules exceed performance targets, eliminating guesswork through explicit criteria and combined effects analysis.

**Hub Document**: This document is part of the Performance Architecture. See Performance Hub for strategic overview and cost model details.

## Performance Budget Breakdown

**Target**: <1ms per event evaluation (local evaluation, excluding network I/O)

**Philosophy**: Guidance-only, not strict allocation

The sub-millisecond target allows minimal intrusion in high-performance/high-load data processing systems capable of handling billions of rows per day. Rather than strict per-operation budgets, focus on:

### Optimization Priorities (Ordered by Impact)

1. **Minimize nesting depth**: Each level adds 128 lookup cost
   - Good: `user_id` (128)
   - Acceptable: `metadata.tenant.id` (384)
   - Review: `a.b.c.d.e.f` (768+)

2. **Prefer simple operators**: Existence checks and equality faster than string operations
   - Fast: `exists` (cost 1), `eq` (cost 5)
   - Moderate: `gt` (cost 7), `in` (cost 8)
   - Slow: `prefix` (cost 10), `suffix` (cost 10)

3. **Avoid nested wildcards**: Each wildcard multiplies execution by 8
   - Good: No wildcards (1×)
   - Acceptable: Single wildcard (8×)
   - Review: Double wildcard (64×), enable sampling
   - Rejected: Triple wildcard (512×+)

4. **Use sampling for expensive rules**: Reduce load without changing logic
   - `sample_rate: 0.01` = 1% sampling = 100× load reduction
   - Use for debug rules, rarely-matching patterns, expensive conditions

### Priority-to-Runtime Relationship

**Important**: Priority is a relative ordering metric, NOT an absolute runtime prediction.

- Higher priority = higher estimated cost
- No strict formula for runtime (depends on data characteristics, array sizes, cache state)
- Use priority for relative ordering between rules, not absolute latency prediction
- Benchmark representative workloads to validate assumptions

### What Priority Value Violates Target?

No fixed threshold (highly data-dependent):

- **General guidance**: Priority >10,000 warrants review
- **Consider**: Rule complexity, wildcard usage, field type, operator choice
- **Mitigation**: Use sampling (`sample_rate`) to reduce load for expensive rules

**Example priority values**:

```
Priority 1,200:  Simple numeric check, no wildcards
Priority 2,500:  Multiple conditions, string operations
Priority 6,000:  Single wildcard with string operations
Priority 32,000: Double wildcard with string operations (review + enable sampling)
```

**Cross-References**:

- Cost Model: Complete cost calculation algorithm
- Sampling: Sampling rate configuration and effectiveness

## Optimization Trade-off Decision Framework

When rule exceeds performance target, use this systematic framework.

### Decision Matrix

| Strategy                  | Use When                               | Pros                                      | Cons                                           |
| ------------------------- | -------------------------------------- | ----------------------------------------- | ---------------------------------------------- |
| **Sampling**              | Rule doesn't need 100% coverage        | Low implementation cost, immediate effect | May miss events                                |
| **Data Restructuring**    | Nested data can be flattened           | Improves performance permanently          | Requires data pipeline changes                 |
| **Rule Splitting**        | Rule has multiple expensive conditions | Reduces per-rule cost                     | More rules to manage                           |
| **Operator Substitution** | `prefix` can be replaced with `eq`     | Lower operator cost                       | May reduce expressiveness                      |
| **Caching**               | Same conditions evaluated repeatedly   | Amortizes cost across events              | Memory overhead, cache invalidation complexity |

### Combined Effects Table

| Optimization                  | Impact on Priority       | Impact on Runtime | Implementation Effort            |
| ----------------------------- | ------------------------ | ----------------- | -------------------------------- |
| Sampling (50%)                | No change                | 50% reduction     | Low (config change)              |
| Flatten 1 nesting level       | -128 per condition       | 10-20% reduction  | High (data pipeline)             |
| Split rule (3 → 2 conditions) | Varies                   | 15-30% reduction  | Medium (rule redesign)           |
| `prefix` → `eq` (exact match) | -5 to -240 per condition | 30-50% reduction  | Low (if semantically equivalent) |
| Add sampling (10%)            | No change                | 90% reduction     | Low (config change)              |

### Decision Criteria (Prioritized)

Follow these criteria in order:

1. **Is rule critical** (cannot miss events)? → Avoid sampling
2. **Can data structure change?** → Consider restructuring (highest long-term value)
3. **Are conditions independent?** → Consider splitting rules
4. **Can operator be simplified?** → Prefer simpler operators first (lowest effort)
5. **Multiple rules with overlapping conditions?** → Consider caching (future optimization)

## Example Decision Tree

Systematic approach for optimizing a rule with priority 32,000 (exceeds 10,000 guideline).

```
Rule has priority 32,000 (exceeds 10,000 guideline)
  ↓
Q: Is this rule critical (must evaluate 100% of events)?
  → Yes: Cannot use sampling, continue
  → No: Add sample_rate: 0.1 (10%), reduces load by 90% ✓
  ↓
Q: Can we change the data pipeline?
  → Yes: Flatten nested structure, reduces priority by 128+ per level ✓
  → No: Continue
  ↓
Q: Can we split rule into multiple simpler rules?
  → Yes: Split complex conditions across rules, reduces per-rule cost ✓
  → No: Continue
  ↓
Q: Can we use simpler operator without losing meaning?
  → Yes: Replace prefix with eq (if exact match acceptable) ✓
  → No: Accept higher cost or reconsider requirements
```

### Decision Examples with Specific Recommendations

**Example 1**: `facilities[*].sensors[*].status starts_with "ALARM-"` (cost 31,104)

**Analysis**:

- Field lookup: 384 (3 string components)
- Operator evaluation: 30,720 (10 × 48 × 64)
- Total: 31,104

**Decision Tree**:

1. Critical rule? If no → **Add `sample_rate: 0.05` (5%)** → Effective cost ~1,555 ✓
2. Can flatten data? If yes → **Flatten to `alarms[*].status`** → Reduces to ~6,272 ✓
3. Can simplify operator? If exact values known → **Replace `starts_with` with `in` operator** → Reduces by 2× ✓

**Recommendation**: Combination approach:

- Flatten data structure (long-term, reduces cost by 75%)
- Add 10% sampling (short-term, reduces load by 90%)

**Example 2**: `tags[*] contains "production"` (cost 6,272)

**Analysis**:

- Field lookup: 128
- Operator evaluation: 6,144 (16 × 48 × 8)
- Total: 6,272

**Decision Tree**:

1. Critical rule? If no → **Add `sample_rate: 0.1` (10%)** → Effective cost ~627 ✓
2. High-throughput pipeline? → **Accept cost, monitor** ✓

**Recommendation**: 10% sampling sufficient for monitoring use case where 100% coverage not critical.

**Example 3**: `temperature > 100` (cost 135)

**Analysis**:

- Field lookup: 128
- Operator evaluation: 7 (7 × 1 × 1)
- Total: 135

**Decision Tree**:

1. Cost < 1,000? → **No optimization needed** ✓

**Recommendation**: Excellent performance, proceed without changes.

## Strategy Details

### Strategy 1: Sampling

**Description**: Reduce evaluation frequency without changing rule logic.

**Configuration**:

```json
{
  "sample_rate": 0.1,  // Evaluate 10% of events
  "rule": { ... }
}
```

**Impact**:

- Priority: No change (priority reflects full cost)
- Runtime: Proportional reduction (10% sampling = 90% load reduction)
- Coverage: Reduced proportionally (10% sampling = 10% coverage)

**When to Use**:

- Debug rules (expensive diagnostics)
- Rarely-matching patterns (low signal-to-noise)
- Monitoring/observability (statistical sampling acceptable)

**When to Avoid**:

- Critical compliance rules (100% coverage required)
- Drop actions (data loss unacceptable)
- High-signal rules (most events match)

### Strategy 2: Data Restructuring

**Description**: Flatten nested arrays or denormalize structure at ingestion time.

**Before**:

```json
{
  "facilities": [{ "id": 1, "sensors": [{ "id": 10, "temp": 95 }] }]
}
```

**After**:

```json
{
  "sensor_readings": [{ "facility_id": 1, "sensor_id": 10, "temp": 95 }]
}
```

**Impact**:

- Priority: -128 per nesting level removed
- Runtime: 10-20% reduction per level
- Maintenance: Data pipeline changes required

**When to Use**:

- High-volume pipelines (optimization pays off)
- Data controlled by your team (can change ingestion)
- Persistent performance issues (long-term solution)

**When to Avoid**:

- Third-party data formats (cannot change upstream)
- Low-volume pipelines (optimization not worth effort)
- Temporary performance issues (sampling faster to implement)

### Strategy 3: Rule Splitting

**Description**: Split rule with multiple expensive conditions into separate rules.

**Before**:

```json
{
  "all": [
    { "field": ["a", "*", "b"], "op": "gt", "value": 100 },
    { "field": ["c", "*", "d"], "op": "eq", "value": "x" },
    { "field": ["e"], "op": "exists" }
  ]
}
```

**After** (two rules):

```json
// Rule 1
{
  "all": [
    {"field": ["a", "*", "b"], "op": "gt", "value": 100},
    {"field": ["e"], "op": "exists"}
  ]
}

// Rule 2
{
  "all": [
    {"field": ["c", "*", "d"], "op": "eq", "value": "x"},
    {"field": ["e"], "op": "exists"}
  ]
}
```

**Impact**:

- Priority: Varies (depends on splitting strategy)
- Runtime: 15-30% reduction (better short-circuit opportunities)
- Maintenance: More rules to manage

**When to Use**:

- Independent conditions (no logical dependencies)
- Different sampling needs (one condition needs higher coverage)
- Different actions (observe vs drop)

**When to Avoid**:

- Tightly coupled conditions (must evaluate together)
- Already many rules (management overhead)

### Strategy 4: Operator Substitution

**Description**: Replace expensive operator with cheaper equivalent where semantically valid.

**Substitution Table**:

| Expensive Operator  | Cheap Replacement                 | Condition          | Cost Reduction |
| ------------------- | --------------------------------- | ------------------ | -------------- |
| `prefix: "ALARM-"`  | `in: ["ALARM-HIGH", "ALARM-LOW"]` | Known values       | ~30%           |
| `suffix: ".json"`   | `in: ["a.json", "b.json"]`        | Known values       | ~30%           |
| `contains: "error"` | `eq: "error"`                     | Exact match        | ~50%           |
| `regex: "^[0-9]+$"` | Type coercion                     | Numeric validation | ~80%           |

**Impact**:

- Priority: -5 to -240 per condition (depends on field type)
- Runtime: 30-50% reduction per substitution
- Expressiveness: May reduce flexibility

**When to Use**:

- Known value set (can enumerate options)
- Exact match acceptable (no partial matching needed)
- Type validation (use type system instead)

**When to Avoid**:

- True pattern matching required
- Unknown value space (cannot enumerate)
- Semantic meaning changes

### Strategy 5: Caching (Future)

**Description**: Cache condition results for repeated evaluations.

**When to Consider** (future implementation):

- Same field paths evaluated across multiple rules
- Same conditions with different actions
- High-frequency evaluation of static data

**Impact** (estimated):

- Priority: No change
- Runtime: 50-80% reduction for cached paths
- Memory: Overhead per cached condition

**Implementation Considerations**:

- Cache invalidation strategy
- Memory pressure management
- Cache hit rate monitoring

**Status**: Deferred to post-MVP. Focus on sampling and data restructuring first.

## Performance Budget for Rule Authors

Practical guidance for rule authors translating technical cost model into actionable recommendations.

### Cost Ranges

| Cost Range   | Performance Level | Description                                   | Example                                         |
| ------------ | ----------------- | --------------------------------------------- | ----------------------------------------------- |
| < 200        | Excellent         | Simple field comparisons                      | `temperature > 100`                             |
| 200-1,000    | Good              | Basic string operations                       | `status == "active"`                            |
| 1,000-5,000  | Moderate          | Single wildcard with strings                  | `tags[*] contains "prod"`                       |
| 5,000-10,000 | Expensive         | Complex string operations with wildcards      | `tags[*] regex "^ALARM-.*"`                     |
| > 10,000     | Very Expensive    | Nested wildcards, requires sampling           | `sensors[*].readings[*] > 100`                  |
| > 30,000     | Critical          | Multiple nested wildcards, mandatory sampling | `facilities[*].sensors[*].tags[*] contains "x"` |

### Sampling Recommendations

**When to use sampling**:

1. **Cost > 10,000**: Consider sampling at 10-50% (`sample_rate: 0.1` to `0.5`)
2. **Cost > 30,000**: Mandatory sampling at 1-10% (`sample_rate: 0.01` to `0.1`)
3. **Cost > 100,000**: Extreme sampling at 0.1-1% (`sample_rate: 0.001` to `0.01`) or redesign rule

**Sampling trade-offs**:

- **Benefit**: Reduces actual evaluation cost proportionally to sample rate
- **Cost**: May miss violations in un-sampled records
- **Best for**: Monitoring and alerting where 100% coverage is not critical
- **Avoid for**: Drop actions where data loss is unacceptable

**Cross-References**:

- Cost Model: Detailed cost calculation for each example
- Sampling: Implementation patterns and sample rate configuration

## Related Documents

**Dependencies** (read these first):

- Cost Model: Provides canonical costs used in decision framework
- Performance Hub: Strategic overview and performance targets

**Related Spokes** (siblings in this hub):

- Sampling: Probabilistic evaluation strategies
- Batch Processing: Vectorization performance characteristics

**Extended by** (documents building on this):

- Rule Lifecycle: Operational controls for performance management
