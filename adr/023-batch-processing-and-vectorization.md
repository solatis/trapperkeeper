# ADR-023: Batch Processing and Vectorization
Date: 2025-10-28

## Context

TrapperKeeper's SDK model (see ADR-002) integrates into data processing pipelines where frameworks like Pandas and Apache Spark operate on batches of data rather than individual records. These frameworks expose vectorized operations (NumPy for Pandas, RDD/DataFrame transformations for Spark) that are orders of magnitude faster than row-by-row iteration.

Key requirements:
- **Performance**: Target <1ms per record evaluation time must scale to batch operations
- **Framework diversity**: Support both in-memory DataFrames (Pandas) and distributed compute (Spark)
- **Memory efficiency**: Batch processing generates many events simultaneously
- **Backpressure handling**: Rule evaluation may be slower than data arrival
- **Consistency**: Vectorized and single-record paths must produce identical results

Traditional row-by-row iteration through large DataFrames negates the performance benefits of vectorized operations and creates bottlenecks in high-throughput pipelines.

## Decision

We will implement **vectorized batch processing** for high-performance data frameworks, where rules are evaluated against entire batches using native vectorized operations.

### 1. Vectorized Rule Evaluation

**Framework-specific implementations**:
- **Pandas**: Apply rules as vectorized NumPy operations on DataFrame columns
- **Spark**: Use DataFrame transformations and column operations for distributed evaluation
- **Base SDKs**: Fall back to optimized sequential evaluation for non-batch frameworks

**Processing model**:
- Process entire batch against each rule as a vectorized operation
- Example: Instead of `for row in df: check_rule(row)`, use `df[df['temperature'] > 100]`
- Leverage framework's native type system and optimizations
- Much faster than row-by-row iteration for large datasets

**Rationale**: Vectorized operations are 10-100x faster than Python loops for numerical data. Pandas/NumPy leverage SIMD instructions and avoid Python interpreter overhead. Spark distributes computation across cluster nodes.

### 2. Event Reporting After Batch Completion

**Buffering strategy**:
- Events buffered in-memory during batch processing
- **Sent to API when all rules have been checked against the entire batch**
  - For single records: After checking all rules against one record/dict
  - For batches (Pandas DataFrame): After checking all rules against entire batch
  - Also triggered on explicit `sensor.flush_events()` or context manager exit

**Memory implications**:
- Large batches may generate thousands of events
- SDK event buffer limits apply (10,000 events, 100MB total - see ADR-002)
- Clear error when buffer limits exceeded: "Event buffer full, flush required"

**Rationale**: Separating evaluation from network I/O allows vectorized operations to run at full speed without blocking on API calls. Batch submission amortizes network overhead across many events.

### 3. Backpressure Handling

**When rule evaluation lags behind data arrival**:
- **Slow down the pipeline** (block on rule evaluation)
- No background buffering or queuing
- Simple synchronous behavior

**Load reduction mechanisms**:
- Use per-rule sampling (`sample_rate` field) to reduce evaluation cost
- Operators can lower sampling rates during load spikes
- No automatic rate adjustment in MVP

**Rationale**: Explicit backpressure prevents unbounded memory growth and keeps system behavior predictable. Sampling provides explicit control over load vs coverage tradeoff. Simple synchronous model avoids complex async coordination.

### 4. Vectorized Type Coercion

**Apply coercion rules to entire columns**:
- Type coercion (see ADR-016) applied vectorially where possible
- Example: `field_type="text"` on numeric column converts entire column to strings in one operation
- Null handling applied vectorially (nulls treated as missing fields)
- Type errors in vectorized operations handled per-element (skip failed elements)

**Compatibility with single-record evaluation**:
- Vectorized and sequential paths must produce identical results
- Test suites verify equivalence on sample datasets
- Field path resolution (see ADR-015) behaves identically

**Rationale**: Vectorized type operations maintain performance benefits. Consistent results across evaluation modes prevent framework-specific bugs.

### 5. Framework Wrapper Architecture

**Separation of concerns**:
- **Base SDKs** (Python/Java/Go): Implement protocol, rule compilation, single-record evaluation
- **Framework wrappers**: Implement vectorized operations specific to framework
  - `sdks/python/trapperkeeper/pandas.py`: Pandas DataFrame operations
  - `sdks/java/trapperkeeper-spark/`: Spark transformation functions
  - Override `api_type` metadata (`"pandas"` or `"spark"`, not base SDK type)

**Wrapper responsibilities**:
- Convert rules to framework-native operations (Pandas masks, Spark filters)
- Handle framework-specific type systems
- Manage framework-specific metadata (DataFrame schema, partition info)
- Delegate to base SDK for protocol, rule sync, event submission

**Rationale**: Framework wrappers leverage base SDK for common functionality while optimizing hot path (rule evaluation) for each framework. Avoid reimplementing protocol logic per framework.

## Consequences

### Benefits

1. **Performance**: Vectorized operations achieve 10-100x speedup over row-by-row iteration on large batches
2. **Framework Native**: Leverages Pandas/Spark optimizations rather than fighting them
3. **Scalability**: Distributed frameworks (Spark) can parallelize rule evaluation across cluster
4. **Memory Efficiency**: Batch event submission amortizes network overhead
5. **Explicit Backpressure**: Synchronous blocking prevents unbounded memory growth
6. **Consistent Results**: Vectorized and sequential paths produce identical outputs
7. **Wrapper Simplicity**: Base SDK handles protocol complexity, wrappers focus on vectorization

### Tradeoffs

1. **Buffer Memory Pressure**: Large batches can generate thousands of events before flush
2. **Framework Coupling**: Must maintain separate wrappers for each batch framework
3. **Testing Complexity**: Must verify equivalence between vectorized and sequential evaluation
4. **Error Handling Subtlety**: Per-element errors in vectorized operations require careful handling
5. **All-or-Nothing Flush**: Cannot report events until entire batch completes
6. **No Automatic Backpressure**: Requires manual sampling adjustments during overload

### Operational Implications

1. **Memory Tuning**: Large batch jobs may need increased event buffer limits
2. **Performance Monitoring**: Vectorized performance orders of magnitude better than sequential
3. **Framework Detection**: Operators can identify framework via `api_type` metadata in events
4. **Sampling Strategy**: High-volume batch jobs benefit from aggressive sampling
5. **Error Investigation**: Type coercion errors in batches require examining DataFrame schemas

## Implementation

1. Implement Pandas wrapper (`sdks/python/trapperkeeper/pandas.py`):
   - Convert conditions to boolean masks (`df['temp'] > 100`)
   - Combine masks with logical operators for OR/AND groups
   - Extract matching rows and generate events
   - Apply sampling using NumPy random selection
   - Override `api_type` to `"pandas"`

2. Implement Spark wrapper (`sdks/java/trapperkeeper-spark/`):
   - Convert rules to Column operations (`col("temp").gt(100)`)
   - Use DataFrame.filter() for rule evaluation
   - Leverage Spark's distributed evaluation across partitions
   - Generate events from filtered DataFrame
   - Override `api_type` to `"spark"`

3. Enforce event buffer limits:
   - Count events during batch processing
   - Raise clear error when buffer limit reached
   - Document buffer size recommendations for common batch sizes
   - Provide configuration for increasing buffer limits

4. Implement backpressure behavior:
   - Block batch processing when event buffer full
   - Document sampling strategies for high-volume workloads
   - Provide clear error messages distinguishing backpressure from other failures

5. Test suite for equivalence:
   - Generate test datasets with diverse schemas
   - Evaluate same rules using vectorized and sequential paths
   - Assert identical event generation
   - Include edge cases: nulls, type coercion, wildcards, missing fields

## Related Decisions

**Depends on:**
- **ADR-002: SDK Model** - Extends the SDK architecture with vectorized operations for data processing frameworks
- **ADR-014: Rule Expression Language** - Applies rule evaluation using vectorized operations on batches

**Also references:**
- **ADR-004: API Service Architecture** - Documents gRPC protocol for event submission
- **ADR-015: Field Path Resolution** - Defines field path semantics for vectorized operations
- **ADR-016: Type System and Coercion** - Defines type coercion rules applied vectorially

## Future Considerations

- **Streaming Frameworks**: Support for Flink, Kafka Streams (micro-batches)
- **Adaptive Buffering**: Automatically adjust buffer size based on batch characteristics
- **Partial Flush**: Report events incrementally during long-running batch operations
- **Async Event Submission**: Background thread submits events while next batch evaluates
- **Columnar Storage**: Native integration with Arrow/Parquet for zero-copy evaluation
- **GPU Acceleration**: CUDA-based vectorized evaluation for massive datasets

## Appendix: Performance Characteristics

### Pandas Vectorized vs Sequential (1M rows)

```python
# Sequential (slow)
for row in df.itertuples():
    if row.temperature > 100:
        events.append(create_event(row))
# Time: ~10 seconds

# Vectorized (fast)
matches = df[df['temperature'] > 100]
events = matches.apply(create_event, axis=1)
# Time: ~100ms (100x faster)
```

### Spark Distributed Evaluation

```java
// Distributed across cluster nodes
Dataset<Row> matches = df.filter(col("temperature").gt(100));
matches.foreach(row -> sensor.reportEvent(row));
// Parallelized across partitions
```

### Event Buffer Sizing Guidelines

| Batch Size | Expected Match Rate | Events Generated | Recommended Buffer |
|------------|---------------------|------------------|-------------------|
| 10K rows   | 1%                  | ~100 events      | 1K (default)      |
| 100K rows  | 1%                  | ~1K events       | 10K (default)     |
| 1M rows    | 1%                  | ~10K events      | 50K (custom)      |
| 10M rows   | 1%                  | ~100K events     | Process in chunks |

**Recommendation**: For batches generating >10K events, either:
1. Increase event buffer size via configuration
2. Process data in smaller chunks with intermediate flushes
3. Increase sampling rate to reduce event volume
