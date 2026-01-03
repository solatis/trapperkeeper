---
doc_type: index
status: active
primary_category: performance
cross_cutting:
  - performance
maintainer: Performance Team
next_review: 2026-02-07
---

# Performance Index

## Purpose

This index provides navigation to all documentation addressing **performance** across the Trapperkeeper system. Use this as a discovery mechanism for performance-related decisions, optimization strategies, and cost model calculations regardless of their primary domain. Performance is critical for achieving <1ms per record evaluation targets in high-throughput streaming pipelines.

## Quick Reference

| Category                | Description                                                         | Key Documents                                                                           |
| ----------------------- | ------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| Cost Model              | Operator costs, field type multipliers, execution cost calculations | [Performance Hub](05-performance/README.md), [Cost Model](05-performance/cost-model.md) |
| Sampling                | Probabilistic sampling strategies for high-throughput optimization  | [Sampling Strategies](05-performance/sampling.md)                                       |
| Batch Processing        | Vectorized operations for Pandas/Spark frameworks                   | [Batch Processing](05-performance/batch-processing.md)                                  |
| Optimization Techniques | Pre-compilation, short-circuit evaluation, nested wildcard limits   | [Optimization Strategies](05-performance/optimization-strategies.md)                    |
| Performance Budget      | Cost thresholds, warning levels, rule complexity limits             | [Performance Hub](05-performance/README.md) Section 4                                   |

## Core Concepts

### Cost Model and Calculation Algorithm

TrapperKeeper uses a deterministic cost model to predict rule evaluation performance. Total cost = lookup cost + execution cost, where lookup cost accounts for field resolution (including wildcards) and execution cost accounts for operator complexity. The cost model enables compile-time performance validation and runtime optimization decisions.

**Relevant Documentation:**

- **[Performance Hub](05-performance/README.md)** - Strategic overview of cost model architecture → See Section 2 for complete cost calculation algorithm
- **[Cost Model](05-performance/cost-model.md)** - Detailed operator costs, field type multipliers, and execution cost multipliers with examples
- **[Rule Expression Language](04-rule-engine/expression-language.md)** - How cost model integrates with DNF evaluation → See Section 4 for performance characteristics
- **[Field Path Resolution](04-rule-engine/field-path-resolution.md)** - Wildcard cost multipliers → See Section 3 for array traversal costs

### Operator Costs

Different operators have different computational costs. String operations (prefix, suffix, in) are more expensive than numeric comparisons (eq, lt, gt). The cost model assigns base costs and execution multipliers to each operator, enabling accurate performance predictions.

**Relevant Documentation:**

- **[Cost Model](05-performance/cost-model.md)** - Complete operator cost table → See Section 2 for operator base costs and execution multipliers
- **[Performance Hub](05-performance/README.md)** - Operator cost categories → See Section 2.3 for operator classification
- **[Type System and Coercion](04-rule-engine/type-system-coercion.md)** - Type coercion impact on operator costs

### Field Structure Multipliers

Field structures affect lookup costs through traversal complexity. Arrays and nested objects require additional traversal compared to simple scalar fields. The cost model uses field structure multipliers to account for this complexity: scalars (1x), arrays (10x), nested objects (10x), arrays of objects (100x). These structure multipliers are distinct from field type multipliers (int, string, boolean, etc.) which affect execution costs based on data type operations.

**Relevant Documentation:**

- **[Performance Hub](05-performance/README.md)** - Field structure multiplier rationale → See Section 2.2 for field complexity impact
- **[Cost Model](05-performance/cost-model.md)** - Field type multipliers (int, string, boolean) → See Section 3 for data type operation costs
- **[Field Path Resolution](04-rule-engine/field-path-resolution.md)** - How field structures affect runtime resolution performance

### Nested Wildcard Limits

Deeply nested wildcards (e.g., `a.*.b.*.c.*`) create exponential cost growth. TrapperKeeper enforces validation limits: maximum 2 wildcards per field path (applies to each field reference independently). These limits prevent performance degradation while supporting common use cases like `events.*.severity` or `logs.*.fields.*.value`.

**Relevant Documentation:**

- **[Optimization Strategies](05-performance/optimization-strategies.md)** - Nested wildcard validation limits → See Section 3 for complete validation rules
- **[Performance Hub](05-performance/README.md)** - Nested wildcard cost analysis → See Section 5 for exponential growth examples
- **[Field Path Resolution](04-rule-engine/field-path-resolution.md)** - Wildcard resolution algorithm → See Section 2 for traversal mechanics
- **[Validation Hub](07-validation/README.md)** - Wildcard validation enforcement → See Section 3.3 for field path validation

### Sampling Strategies

Probabilistic sampling enables rule evaluation on high-throughput streams where evaluating every record exceeds performance budgets. TrapperKeeper supports deterministic sampling (every Nth record) and hash-based sampling (consistent per-key sampling). Sampling configuration is rule-specific with per-rule sampling rates.

**Relevant Documentation:**

- **[Sampling Strategies](05-performance/sampling.md)** - Complete sampling algorithm specifications and trade-offs
- **[Performance Hub](05-performance/README.md)** - Sampling overview → See Section 3 for sampling rationale
- **[Rule Lifecycle](04-rule-engine/lifecycle.md)** - How sampling configuration integrates with rule states

### Batch Processing and Vectorization

Batch processing optimizes rule evaluation for data processing frameworks like Pandas and Spark. Vectorized operations amortize rule compilation costs across thousands of records, achieving 10-100x throughput improvements. Pre-compiled rules evaluate entire DataFrames/RDDs in single operations rather than row-by-row iteration.

**Relevant Documentation:**

- **[Batch Processing](05-performance/batch-processing.md)** - Vectorization strategies for Pandas/Spark with performance benchmarks
- **[Performance Hub](05-performance/README.md)** - Batch processing benefits -> See Section 6 for vectorization impact
- **[SDK Model](02-architecture/sdk-model.md)** - Pre-compilation architecture enabling batch optimization

### Pre-Compilation and Short-Circuit Evaluation

Rules pre-compile to nested predicates at rule sync time, eliminating runtime parsing overhead. Evaluation uses short-circuit logic: DNF evaluation stops at first matching OR group, AND evaluation stops at first failed condition. Pre-compilation + short-circuit evaluation are the primary techniques achieving <1ms targets.

**Relevant Documentation:**

- **[Optimization Strategies](05-performance/optimization-strategies.md)** - Pre-compilation mechanics → See Section 1 for compilation pipeline
- **[Performance Hub](05-performance/README.md)** - Short-circuit evaluation strategy → See Section 7 for evaluation order optimization
- **[Rule Expression Language](04-rule-engine/expression-language.md)** - DNF structure enabling short-circuit → See Section 2 for evaluation semantics

### Performance Budget and Thresholds

TrapperKeeper enforces performance budgets at rule creation and compilation time. Rules exceeding hard limits (>2 nested wildcards) are rejected with 400 errors by the API. Rules exceeding soft limits (>1 nested wildcard without sampling enabled) trigger warnings but are accepted. Budget enforcement prevents performance regressions from poorly constructed rules.

**Relevant Documentation:**

- **[Performance Hub](05-performance/README.md)** - Complete performance budget guidance → See Section 4 for cost thresholds and warning levels
- **[Cost Model](05-performance/cost-model.md)** - How cost calculations drive budget decisions → See Section 5 for budget examples
- **[Validation Hub](07-validation/README.md)** - Performance budget validation → See Section 3.6 for rule complexity limits

## Domain Coverage Matrix

| Domain         | Coverage | Key Document                                                                    |
| -------------- | -------- | ------------------------------------------------------------------------------- |
| Architecture   | ✓        | [SDK Model](02-architecture/sdk-model.md)                                       |
| API Design     | ✓        | [API Service](02-architecture/api-service.md) (sync performance)                |
| Database       | ✓        | [Database Backend](09-operations/database-backend.md) (query performance)       |
| Security       | ✓        | [Security Hub](06-security/README.md) (timing attack considerations)            |
| Performance    | ✓        | [Performance Hub](05-performance/README.md)                                     |
| Validation     | ✓        | [Validation Hub](07-validation/README.md) (validation cost)                     |
| Configuration  | ✗        | N/A (configuration loading is startup-only)                                     |
| Testing        | ✓        | [Testing Philosophy](01-principles/testing-philosophy.md) (performance testing) |
| Deployment     | ✗        | N/A (deployment is operational concern)                                         |
| Error Handling | ✓        | [Error Taxonomy](08-resilience/error-taxonomy.md) (error handling overhead)     |

## Patterns and Best Practices

### Pre-Compilation Pattern

**Description**: Rules compile to optimized predicates at sync time rather than runtime. Compilation happens once per rule version; evaluation happens millions of times. Pre-compilation eliminates parsing overhead, enables type checking, and allows cost validation before production deployment.

**Used In**:

- [Optimization Strategies](05-performance/optimization-strategies.md) Section 1
- [Performance Hub](05-performance/README.md) Section 7
- [SDK Model](02-architecture/sdk-model.md) Section 2

### Short-Circuit Evaluation Pattern

**Description**: DNF structure enables aggressive short-circuiting. OR groups evaluate left-to-right, stopping at first match. AND conditions within groups evaluate left-to-right, stopping at first failure. Condition ordering (cheap checks first) maximizes short-circuit benefits.

**Used In**:

- [Optimization Strategies](05-performance/optimization-strategies.md) Section 2
- [Performance Hub](05-performance/README.md) Section 7
- [Rule Expression Language](04-rule-engine/expression-language.md) Section 2

### Cost-Based Validation Pattern

**Description**: Cost model enables compile-time performance validation. Rules exceeding budget are rejected before production deployment. Warning thresholds provide feedback for optimization. Cost-based validation prevents performance regressions from poorly constructed rules.

**Used In**:

- [Performance Hub](05-performance/README.md) Section 4
- [Cost Model](05-performance/cost-model.md) Section 5
- [Validation Hub](07-validation/README.md) Section 3.6

### Sampling for High-Throughput Pattern

**Description**: Probabilistic sampling enables rule evaluation on streams exceeding evaluation capacity. Deterministic sampling (every Nth record) and hash-based sampling (consistent per-key) provide different trade-offs. Sampling configuration is rule-specific, enabling mixed strategies (critical rules at 100%, less critical rules at 10%).

**Used In**:

- [Sampling Strategies](05-performance/sampling.md)
- [Performance Hub](05-performance/README.md) Section 3
- [Rule Lifecycle](04-rule-engine/lifecycle.md)

### Vectorization for Batch Processing Pattern

**Description**: Batch processing frameworks (Pandas, Spark) amortize rule compilation costs across thousands of records. Pre-compiled rules evaluate entire DataFrames/RDDs in vectorized operations. Vectorization achieves 10-100x throughput improvements compared to row-by-row evaluation.

**Used In**:

- [Batch Processing](05-performance/batch-processing.md)
- [Performance Hub](05-performance/README.md) Section 6
- [SDK Model](02-architecture/sdk-model.md) Section 3

## Related Indexes

- **[Validation Index](validation-index.md)**: Validation has performance implications. See validation index for cost of validation layers and how validation integrates with performance budgets.
- **[Error Handling Index](error-handling-index.md)**: Error handling has performance overhead. See error handling index for performance considerations in error recovery and logging.
- **[Observability Index](observability-index.md)**: Observability (logging, tracing, metrics) has performance overhead. See observability index for structured logging performance characteristics.

## Maintenance Notes

**Next Review**: 2026-02-07 (quarterly)
**Maintainer**: Performance Team

**Known Gaps**:

- Performance profiling guide for rule authors
- Real-world performance case studies from production deployments

**Planned Additions**:

- Benchmark suite implementation and documentation
- Performance testing framework integration with CI/CD
- Automated performance regression detection
- Performance optimization cookbook with common patterns
