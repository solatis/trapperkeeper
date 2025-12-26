---
doc_type: spoke
status: active
date_created: 2025-11-07
primary_category: performance
hub_document: /Users/lmergen/git/trapperkeeper/doc/05-performance/README.md
tags:
  - batch-processing
  - vectorization
  - pandas
  - spark
---

# Batch Processing and Vectorization

## Context

TrapperKeeper's SDK model integrates into data processing pipelines where frameworks like Pandas and Apache Spark operate on batches of data rather than individual records. These frameworks expose vectorized operations (NumPy for Pandas, RDD/DataFrame transformations for Spark) that are orders of magnitude faster than row-by-row iteration. Vectorized batch processing provides 10-100× speedup over sequential evaluation.

**Hub Document**: This document is part of the Performance Architecture. See Performance Hub for strategic overview and sampling strategies.

## Vectorized Rule Evaluation

Framework-specific implementations leverage native vectorized operations for maximum performance.

### Framework-Specific Implementations

- **Pandas**: Apply rules as vectorized NumPy operations on DataFrame columns
- **Spark**: Use DataFrame transformations and column operations for distributed evaluation
- **Base SDKs**: Fall back to optimized sequential evaluation for non-batch frameworks

### Processing Model

**Sampling applied BEFORE rule evaluation** (see Sampling document for detailed rationale):

- Sample subset of rows first, then evaluate rule conditions only against sampled subset
- Example for 1% sampling: `sampled = df.sample(frac=0.01); matches = sampled[sampled['temperature'] > 100]`
- Alternative (vectorized mask): `mask = np.random.random(len(df)) < 0.01; matches = df[mask & (df['temp'] > 100)]`
- Choose fastest implementation approach (typically explicit sampling first)
- Leverage framework's native type system and optimizations
- Much faster than row-by-row iteration for large datasets

**Rationale**: Vectorized operations are 10-100x faster than Python loops for numerical data. Pandas/NumPy leverage SIMD instructions and avoid Python interpreter overhead. Spark distributes computation across cluster nodes. Sampling before evaluation maximizes performance benefit by reducing the number of rows that undergo expensive condition checks.

**Cross-References**:

- Sampling: Complete sampling implementation patterns
- SDK Model: Pre-compilation architecture

## Event Reporting After Batch Completion

Buffering strategy separates evaluation from network I/O for maximum performance.

### Buffering Strategy

**Events buffered in-memory during batch processing**:

- Sent to API when all rules have been checked against the entire batch
  - For single records: After checking all rules against one record/dict
  - For batches (Pandas DataFrame): After checking all rules against entire batch
  - Also triggered on explicit `sensor.flush_events()` or context manager exit

### Memory Implications

**Large batches may generate many events**:

- SDK event buffer limits apply (128 events, 1MB per event, 128MB total)
- **Auto-flush implemented**: automatically chunks large batches (process 128 events → flush → next 128)
- Clear error when per-event size limit exceeded: Log warning, remove source record, drop if still too large

**Rationale**: Separating evaluation from network I/O allows vectorized operations to run at full speed without blocking on API calls. Batch submission amortizes network overhead across many events.

**Cross-References**:

- SDK Model: Buffer management and flush semantics
- Event Schema: Event structure and storage format

## Backpressure Handling

Simple synchronous behavior prevents unbounded memory growth.

### When Rule Evaluation Lags Behind Data Arrival

**Slow down the pipeline** (block on rule evaluation):

- No background buffering or queuing
- Simple synchronous behavior

### Load Reduction Mechanisms

**Use per-rule sampling** (`sample_rate` field) to reduce evaluation cost:

- Operators can lower sampling rates during load spikes
- No automatic rate adjustment in MVP

**Rationale**: Explicit backpressure prevents unbounded memory growth and keeps system behavior predictable. Sampling provides explicit control over load vs coverage tradeoff. Simple synchronous model avoids complex goroutine coordination.

**Cross-References**:

- Sampling: Sampling rate configuration

## Vectorized Type Coercion

Type coercion rules applied to entire columns for maximum performance.

### Apply Coercion Rules to Entire Columns

**Type coercion applied vectorially where possible**:

- Example: `field_type="text"` on numeric column converts entire column to strings in one operation
- **Null-like values**: Handled vectorially (nulls treated as missing fields per `on_missing_field` policy)
- **Type coercion failures**: Handled per-element (treated as condition failed, NOT missing field - elements with failed coercion evaluate to false)

### Critical Distinction (Consistent with Type System)

- **Null values** (e.g., `pd.NA`, `None`) → Treated as missing field, defer to `on_missing_field` policy
- **Coercion failures** (e.g., `"invalid"` → numeric) → Condition fails for that element, continue evaluation

### Compatibility with Single-Record Evaluation

**Vectorized and sequential paths must produce identical results**:

- Test suites verify equivalence on sample datasets
- Field path resolution behaves identically

**Rationale**: Vectorized type operations maintain performance benefits while preserving the distinction between null values and coercion failures. Consistent results across evaluation modes prevent framework-specific bugs.

**Cross-References**:

- Type System: Coercion rules and null handling semantics
- Field Path Resolution: Field path semantics

## Framework Wrapper Architecture

Separation of concerns between base SDKs and framework-specific optimizations.

### Separation of Concerns

**Base SDKs** (Python/Java/Go): Implement protocol, rule compilation, single-record evaluation

**Framework wrappers**: Implement vectorized operations specific to framework

- `sdks/python/trapperkeeper/pandas.py`: Pandas DataFrame operations
- `sdks/java/trapperkeeper-spark/`: Spark transformation functions
- Override `api_type` metadata (`"pandas"` or `"spark"`, not base SDK type)

### Wrapper Responsibilities

**Wrappers handle framework-specific concerns**:

- Convert rules to framework-native operations (Pandas masks, Spark filters)
- Handle framework-specific type systems
- Manage framework-specific metadata (DataFrame schema, partition info)
- Delegate to base SDK for protocol, rule sync, event submission

**Rationale**: Framework wrappers leverage base SDK for common functionality while optimizing hot path (rule evaluation) for each framework. Avoid reimplementing protocol logic per framework.

**Cross-References**:

- SDK Model: Base SDK architecture
- Monorepo Structure: SDK organization and wrapper locations

## Implementation Patterns

### Pandas Wrapper Implementation

```python
# sdks/python/trapperkeeper/pandas.py

def evaluate_rule_pandas(df, rule):
    # Apply sampling BEFORE evaluation
    if rule.sample_rate == 0.0:
        return pd.DataFrame()

    if rule.sample_rate < 1.0:
        sampled_df = df.sample(frac=rule.sample_rate)
    else:
        sampled_df = df

    # Convert conditions to boolean masks on sampled subset
    mask = sampled_df['temp'] > 100

    # Extract matching rows and generate events
    matches = sampled_df[mask]
    return matches
```

**Override `api_type` to `"pandas"`** to indicate framework-specific processing.

### Spark Wrapper Implementation

```java
// sdks/java/trapperkeeper-spark/

public Dataset<Row> evaluateRuleSpark(Dataset<Row> df, Rule rule) {
    // Apply sampling BEFORE evaluation
    if (rule.getSampleRate() == 0.0) {
        return df.sparkSession().emptyDataFrame();
    }

    Dataset<Row> sampled;
    if (rule.getSampleRate() < 1.0) {
        sampled = df.sample(false, rule.getSampleRate());
    } else {
        sampled = df;
    }

    // Convert rules to Column operations on sampled subset
    Column condition = col("temp").gt(100);

    // Use DataFrame.filter() for rule evaluation on sampled data
    return sampled.filter(condition);
}
```

**Override `api_type` to `"spark"`** to indicate distributed processing.

**Cross-References**:

- Rule Expression Language: Rule structure and semantics
- Sampling: Sampling implementation patterns

## Event Buffer Limits and Auto-Flush

### Buffer Size Configuration

**SDK event buffer limits**:

- 128 events default
- 1MB per-event size limit
- 128MB total memory cap
- Limits configurable per sensor initialization

### Auto-Flush Behavior

**Automatically chunk large batches**:

- Process 128 events → flush → next 128
- Per-event size limit: 1MB (1,048,576 bytes) in native representation
- Handle oversized events: log warning, remove source record, drop if still too large

**Cross-References**:

- SDK Model: Complete buffer management specification
- Client Metadata: Size limits for metadata namespace

## Performance Characteristics

### Pandas Vectorized vs Sequential (1M rows)

```python
# Sequential (slow)
for row in df.itertuples():
    if row.temperature > 100:
        events.append(create_event(row))
# Time: ~10 seconds

# Vectorized without sampling (fast)
matches = df[df['temperature'] > 100]
events = matches.apply(create_event, axis=1)
# Time: ~100ms (100x faster)

# Vectorized with sampling BEFORE evaluation (fastest)
# Option A: Explicit sampling first (typically faster)
sampled = df.sample(frac=0.01)  # Sample 1% of rows first
matches = sampled[sampled['temperature'] > 100]
events = matches.apply(create_event, axis=1)
# Time: ~1ms (100x faster than no sampling, evaluates 1% of rows)

# Option C: Vectorized boolean mask (alternative)
sample_mask = np.random.random(len(df)) < 0.01
matches = df[sample_mask & (df['temperature'] > 100)]
events = matches.apply(create_event, axis=1)
# Time: ~50ms (may still compute temperature condition for all rows)
```

### Spark Distributed Evaluation

```java
// Distributed across cluster nodes WITH sampling before evaluation
// Sample BEFORE applying filter (reduces work across cluster)
Dataset<Row> sampled = df.sample(false, 0.01); // 1% sample without replacement
Dataset<Row> matches = sampled.filter(col("temperature").gt(100));
matches.foreach(row -> sensor.reportEvent(row));
// Parallelized across partitions, but only evaluates 1% of data
```

### Event Buffer Sizing Guidelines

| Batch Size | Expected Match Rate | Events Generated | Auto-Flush Behavior             |
| ---------- | ------------------- | ---------------- | ------------------------------- |
| 10K rows   | 1%                  | ~100 events      | Single flush after batch        |
| 100K rows  | 1%                  | ~1K events       | ~8 auto-flushes (128 per flush) |
| 1M rows    | 1%                  | ~10K events      | ~78 auto-flushes                |
| 10M rows   | 1%                  | ~100K events     | ~781 auto-flushes               |

**Auto-flush behavior**: SDK library automatically flushes when buffer reaches 128 events. This enables processing arbitrarily large batches without manual chunking. The 128-event limit (with 1MB per-event cap) balances memory usage with network efficiency.

**Configuration**: Buffer size (128 events default) and per-event size limit (1MB default) are configurable per sensor, allowing higher limits when needed.

**Cross-References**:

- SDK Model: Buffer configuration options
- Sampling: Sampling effectiveness analysis

## Edge Cases and Limitations

**Known Limitations**:

- **Buffer Memory Pressure**: Large batches can generate thousands of events before flush
- **Framework Coupling**: Must maintain separate wrappers for each batch framework
- **Testing Complexity**: Must verify equivalence between vectorized and sequential evaluation
- **Error Handling Subtlety**: Per-element errors in vectorized operations require careful handling
- **All-or-Nothing Flush**: Cannot report events until entire batch completes
- **No Automatic Backpressure**: Requires manual sampling adjustments during overload

**Edge Cases**:

- **Empty DataFrame**: No events generated, no flush triggered
- **All rows filtered**: Events buffer remains empty
- **Oversized single event**: Logged warning, event dropped
- **DataFrame with non-standard types**: Type coercion may fail per-element

**Cross-References**:

- Type System: Type coercion failure handling
- Resilience Hub: Error handling for batch processing failures

## Related Documents

**Dependencies** (read these first):

- SDK Model: Extends the SDK architecture with vectorized operations for data processing frameworks
- Rule Expression Language: Applies rule evaluation using vectorized operations on batches

**Related Spokes** (siblings in this hub):

- Sampling: Sampling strategies applied before batch evaluation
- Cost Model: Performance implications of vectorization
- Optimization Strategies: Trade-offs between vectorization and other optimizations

**Extended by** (documents building on this):

- Field Path Resolution: Defines field path semantics for vectorized operations
- Type System: Defines type coercion rules applied vectorially
- Monorepo Structure: Organizes Pandas and Spark wrapper locations in SDK directories
