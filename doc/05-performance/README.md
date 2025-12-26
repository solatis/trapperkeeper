---
doc_type: hub
status: active
date_created: 2025-11-07
primary_category: performance
consolidated_spokes:
  - cost-model.md
  - optimization-strategies.md
  - sampling.md
  - batch-processing.md
tags:
  - performance
  - optimization
  - cost-model
  - sampling
---

# Performance Architecture

## Context

Performance strategy was previously fragmented across multiple rule engine and SDK ADRs with significant duplication and inconsistency. The cost calculation algorithm appeared in two locations with 100% duplication. Field type multipliers were documented in three separate places. Nested wildcard validation guidance was duplicated at 95% overlap. This fragmentation created maintenance burden, terminology inconsistencies ("cost multiplier" vs "execution multiplier"), and prevented developers from getting a unified view of performance considerations.

TrapperKeeper's performance requirements are demanding: sub-millisecond per-event evaluation for high-performance data processing, support for billions of events per day in industrial IoT and financial data pipelines, and minimal intrusion in systems capable of processing massive data volumes. The schema-agnostic architecture and ephemeral sensor model constrain optimization strategies—no pre-compilation based on known schemas, no long-running query plan learning.

This hub consolidates all performance guidance into a single strategic overview with clear navigation to detailed implementations. It eliminates duplication by establishing canonical definitions and constants, standardizes terminology across all documents, and provides comprehensive trade-off guidance for performance optimization decisions.

## Decision

We will implement a **unified performance strategy** establishing canonical cost models, standardized terminology, and comprehensive optimization guidance from SDK design through rule evaluation.

This document serves as the performance hub providing strategic overview of TrapperKeeper's performance architecture. It consolidates performance considerations from rule expression language, field resolution, type system, sampling strategies, and batch processing into a cohesive strategy with cross-references to detailed implementation documents.

### Performance Philosophy and Targets

**Target**: Sub-millisecond per-event evaluation (local evaluation, excluding network I/O) enables minimal intrusion in high-performance data processing systems capable of handling billions of rows per day.

**Core Principle**: Performance guidance rather than strict allocation. Rather than enforcing hard per-operation budgets, TrapperKeeper prioritizes:

1. **Minimize nesting depth**: Each field path component adds lookup cost
2. **Prefer simple operators**: Existence checks and equality faster than string operations
3. **Avoid nested wildcards**: Each wildcard multiplies execution cost by 8
4. **Use sampling for expensive rules**: Reduce load without changing logic

**Priority-to-Runtime Relationship**: Priority is a relative ordering metric, NOT an absolute runtime prediction. Higher priority indicates higher estimated cost. Use priority for ordering rules (cheapest first), not for predicting exact latency. Actual runtime depends on data characteristics, array sizes, and cache state.

**General Guidance**: Priority >10,000 warrants review. Consider rule complexity, wildcard usage, field type, and operator choice. Enable sampling (`sample_rate`) for expensive rules to reduce load while maintaining rule semantics.

**Cross-References**:

- Cost Model: Canonical constants and calculation algorithms
- Optimization Strategies: Trade-off decision framework and combined effects
- Architectural Principles (Principle 2): Least intrusive by default

### Cost Model and Canonical Constants

The cost model establishes single source of truth for all performance calculations, preventing duplication and enabling consistent updates. All performance constants (operator costs, field type multipliers, execution multipliers) are defined canonically in the Cost Model document.

**Four Distinct Cost Components** (standardized terminology):

1. **Lookup Cost**: Cost of resolving field paths in nested structures (128 per string component)
2. **Execution Cost**: Cost of executing rule conditions with array expansion (base cost × execution multiplier)
3. **Operator Cost**: Base cost of operator evaluation (1 for existence checks, 5-10 for comparisons)
4. **Field Type Multiplier**: Cost adjustment based on type complexity (1× for int/boolean, 48× for string, 128× for any)

**Complete Cost Formula**:

```
condition_cost = field_lookup_cost + operator_evaluation_cost

field_lookup_cost = 128 × string_component_count
operator_evaluation_cost = operator_base_cost × field_type_mult × execution_mult
```

**Execution Multiplier**: `8^n` where n = nested wildcard count. Reflects exponential growth of nested iteration. Examples: no wildcards = 1×, single wildcard = 8×, double nesting = 64×, triple nesting = 512× (REJECTED by validation).

**Validation Limits**:

- **Hard limit**: Maximum 2 nested wildcards per field path (API rejects with 400 error)
- **Soft limit**: Maximum 1 nested wildcard without sampling (rule engine warns when `sample_rate` not enabled or = 1.0)
- **field_ref operator**: ZERO wildcards allowed (must reference exact field)

**Cross-References**:

- Cost Model: Complete operator costs, field type multipliers, calculation algorithm with examples
- Rule Expression Language: Priority calculation integration
- Field Path Resolution: Nested wildcard limits and validation

**Example**: `facilities[*].sensors[*].status starts_with "ALARM-"` has cost 31,104 (384 lookup + 30,720 operator evaluation with 2 nested wildcards). Recommendation: Enable sampling at 1-10% (`sample_rate: 0.01` to `0.1`).

### Optimization Strategies and Trade-offs

Performance optimization follows a systematic decision framework balancing sampling, data restructuring, rule splitting, operator substitution, and caching strategies.

**Decision Matrix**: When rule exceeds performance target, use this framework:

| Strategy              | Use When                               | Pros                                      | Cons                                           |
| --------------------- | -------------------------------------- | ----------------------------------------- | ---------------------------------------------- |
| Sampling              | Rule doesn't need 100% coverage        | Low implementation cost, immediate effect | May miss events                                |
| Data Restructuring    | Nested data can be flattened           | Improves performance permanently          | Requires data pipeline changes                 |
| Rule Splitting        | Rule has multiple expensive conditions | Reduces per-rule cost                     | More rules to manage                           |
| Operator Substitution | `prefix` can be replaced with `eq`     | Lower operator cost                       | May reduce expressiveness                      |
| Caching               | Same conditions evaluated repeatedly   | Amortizes cost across events              | Memory overhead, cache invalidation complexity |

**Combined Effects Table**:

| Optimization                  | Impact on Priority       | Impact on Runtime | Implementation Effort            |
| ----------------------------- | ------------------------ | ----------------- | -------------------------------- |
| Sampling (50%)                | No change                | 50% reduction     | Low (config change)              |
| Flatten 1 nesting level       | -128 per condition       | 10-20% reduction  | High (data pipeline)             |
| Split rule (3 → 2 conditions) | Varies                   | 15-30% reduction  | Medium (rule redesign)           |
| `prefix` → `eq`               | -5 to -240 per condition | 30-50% reduction  | Low (if semantically equivalent) |
| Add sampling (10%)            | No change                | 90% reduction     | Low (config change)              |

**Decision Criteria** (prioritized):

1. Is rule critical (cannot miss events)? → Avoid sampling
2. Can data structure change? → Consider restructuring (highest long-term value)
3. Are conditions independent? → Consider splitting rules
4. Can operator be simplified? → Prefer simpler operators first (lowest effort)
5. Multiple rules with overlapping conditions? → Consider caching (future)

**Cross-References**:

- Optimization Strategies: Complete decision tree with examples
- Cost Model: Performance budget breakdown
- Sampling: Probabilistic evaluation strategies

**Example**: Rule with priority 32,000 (exceeds guideline). If critical (100% coverage required), cannot use sampling. If data pipeline can change, flatten nested structure (reduces priority by 128+ per level). Otherwise, split into simpler rules or accept higher cost.

### Sampling and Probabilistic Evaluation

Probabilistic sampling enables flexible performance control through per-rule `sample_rate` configuration, allowing expensive rules to coexist with high-throughput pipelines.

**Sampling Strategy**: Random sampling applied BEFORE rule evaluation (reduces both field extraction and condition evaluation overhead). Per-rule configuration allows mixing debug rules (low sampling) with production rules (high sampling) in same sensor.

**Sample Rate Configuration**: Float value 0.0-1.0 (default 1.0 = 100% evaluation)

- 0.0 = always skip (RNG bypassed for performance)
- 0.1 = 10% sampling (90% load reduction)
- 1.0 = 100% evaluation (no sampling, RNG bypassed)

**Cost-Based Predicate Ordering**: Within each rule's condition groups, conditions are reordered by estimated cost before execution (cheapest first). Combined with optional short-circuit evaluation, this maximizes probability of early exit for failed conditions.

**Cost Ranking** (cheapest to most expensive):

1. Existence checks (`exists`, `is_null`): minimal operator cost (1)
2. Equality with primitives (`eq`, `neq` with boolean/numeric): operator cost 5, type multiplier 1
3. Numeric comparisons (`lt`, `gt`, etc.): operator cost 7, type multiplier 1
4. Equality with strings: operator cost 5, type multiplier 48
5. String operations (`prefix`, `suffix`): operator cost 10, type multiplier 48
6. Single wildcard evaluation: execution multiplier 8
7. Nested wildcard evaluation: execution multiplier 64 (double nesting), requires sampling

**Sampling Recommendations**:

- Cost > 10,000: Consider sampling at 10-50%
- Cost > 30,000: Mandatory sampling at 1-10%
- Cost > 100,000: Extreme sampling at 0.1-1% or redesign rule

**Cross-References**:

- Sampling: Implementation details, short-circuit evaluation patterns
- Cost Model: Cost estimation for condition ordering
- Performance Model: Performance budget guidance

**Example**: Rule `tags[*] contains "production"` has cost 6,272. With `sample_rate: 0.1` (10% sampling), effective cost reduces to ~627 per record. Good trade-off for monitoring use case where 100% coverage not critical.

### Batch Processing and Vectorization

Vectorized batch processing provides 10-100× speedup over row-by-row iteration for data frameworks like Pandas and Apache Spark through native vectorized operations.

**Framework-Specific Implementations**:

- **Pandas**: Apply rules as vectorized NumPy operations on DataFrame columns
- **Spark**: Use DataFrame transformations and column operations for distributed evaluation
- **Base SDKs**: Fall back to optimized sequential evaluation for non-batch frameworks

**Processing Model**:

1. **Sampling BEFORE evaluation**: Sample subset of rows first, then evaluate rule conditions only against sampled subset
2. **Vectorized operations**: Leverage framework's native type system and optimizations (NumPy SIMD instructions, Spark distributed compute)
3. **Event buffering**: Buffer events in-memory during batch processing, sent to API when all rules checked against entire batch
4. **Auto-flush**: SDK automatically chunks large batches (process 128 events → flush → next 128) to prevent memory exhaustion

**Memory Implications**:

- Event buffer limits: 128 events, 1MB per event, 128MB total (configurable)
- Auto-flush prevents manual chunking for large batches
- Clear error handling when per-event size limit exceeded

**Vectorized Type Coercion**: Apply coercion rules to entire columns (per type system), handling null-like values and coercion failures vectorially.

- **Null values** (`pd.NA`, `None`): Treated as missing field, defer to `on_missing_field` policy
- **Coercion failures** (`"invalid"` → numeric): Condition fails for that element, continue evaluation

**Framework Wrappers**: Separate from base SDKs, override `api_type` metadata (`"pandas"` or `"spark"`) to indicate framework-specific processing.

**Cross-References**:

- Batch Processing: Complete vectorized implementation patterns, backpressure handling
- SDK Model: Ephemeral sensors and pre-compilation architecture
- Type System: Coercion rules applied vectorially

**Example**: Pandas rule on 1M rows with `sample_rate: 0.01` (1% sampling). Explicit sampling first (`sampled = df.sample(frac=0.01); matches = sampled[sampled['temp'] > 100]`) evaluates only 10K rows (~1ms) vs evaluating all 1M rows (~100ms).

## Consequences

**Benefits**:

- Single source of truth for all performance calculations eliminates duplication and drift
- Standardized terminology (lookup cost, execution cost, operator cost, field type multiplier) prevents confusion
- Reduced maintenance burden through centralized constants and formulas
- Clear trade-off decision framework helps developers optimize expensive rules systematically
- Predictable performance through cost model enables priority-based optimization before deployment
- Hub architecture reduces coupling by having spoke documents reference this hub for canonical values
- Comprehensive validation prevents pathological performance cases (nested wildcard limits)
- Tunable constants can be adjusted based on production benchmarks with documented rationale

**Trade-offs**:

- Hub dependency means all performance-related documents depend on this hub for canonical values
- Priority opacity: dynamically calculated priority still invisible to users (not changed by this strategy)
- Estimated costs: priority reflects estimated cost, not actual runtime (data-dependent)
- Constant tuning burden: initial values may need adjustment based on production profiling
- Framework variation: constants based on x86_64 benchmarks may not apply to ARM or other architectures
- Non-deterministic sampling: random sampling means some records skip rules unpredictably (acceptable for observability)
- No cross-rule optimization: each rule evaluated independently (cannot deduplicate field extractions)

## Related Documents

**Consolidated Spokes** (this hub consolidates):

- Cost Model: Maps to Canonical Cost Model section (operator costs, field type multipliers, calculation algorithm)
- Optimization Strategies: Maps to Optimization Strategies section (decision framework, combined effects table)
- Sampling: Maps to Sampling and Probabilistic Evaluation section (sampling strategies, cost-based ordering)
- Batch Processing: Maps to Batch Processing section (vectorization, framework wrappers)

**Dependencies** (foundational documents):

- Architectural Principles (Principle 2): Implements least intrusive principle through performance guidance
- Rule Expression Language: Integrates cost model into priority calculation for rule ordering

**References** (related hubs/documents):

- Validation Hub: Nested wildcard validation layer specifications
- Type System: Field type multiplier rationale and coercion cost implications
- Field Path Resolution: Field path resolution details and wildcard semantics
- SDK Model: Pre-compilation architecture and ephemeral sensor lifecycle

**Extended by**:

- Rule Lifecycle: Operational controls for performance management (dry-run mode, rule states)
