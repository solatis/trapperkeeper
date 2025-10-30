# ADR-019: Event Schema and Storage Architecture

## Revision Log

| Date | Description |
|------|-------------|
| 2025-10-28 | Document created |

## Context

TrapperKeeper captures events when rules match data in processing pipelines. These events must handle high-volume ingestion, provide point-in-time auditability, support schema-agnostic storage with append-only semantics, and enable migration from MVP file-based storage to production time-series databases.

## Decision

We will implement a **JSONL file-based storage system** for MVP with a comprehensive event schema that captures complete context. Events are stored using hour-bucketed JSONL files with atomic rotation, single-writer concurrency model, and asynchronous compression. The implementation uses gRPC with Protocol Buffers for the wire protocol while documenting the logical schema structure in JSON notation for readability.

## Consequences

### Benefits

1. **Auditability**: Full rule and record snapshots enable point-in-time reconstruction
2. **Debugging**: Complete event context eliminates need for external data correlation
3. **Simplicity**: JSONL files are human-readable, grep-able, and require no database
4. **Honest Scale**: File-based storage clearly communicates MVP limitations
5. **Tooling Friendly**: Standard format works with jq, grep, zcat, and streaming tools
6. **Migration Path**: Clear evolution to TimescaleDB, InfluxDB, or QuasarDB
7. **Deduplication**: Client-generated UUIDv7 enables idempotent event ingestion
8. **Type Safety**: Protobuf wire protocol catches schema errors at compile time
9. **Performance**: Binary protocol minimizes serialization overhead for high-volume sensors

### Tradeoffs

1. **Storage Bloat**: Capturing full records and rules increases storage requirements significantly
2. **Query Limitations**: No indexed search, aggregations, or complex filtering without external tools
3. **Scale Ceiling**: JSONL files impractical beyond 10-100 million events (requires migration)
4. **No Transactions**: Cannot atomically query across multiple hour files
5. **Compression Latency**: Current hour uncompressed until rotation (larger disk footprint)
6. **Single Instance Only**: File-based locking prevents horizontal scaling (MVP constraint)
7. **Binary Protocol Opacity**: Protobuf harder to debug than JSON (mitigated by grpcurl, reflection)

### Operational Implications

1. **Disk Management**: Operators must monitor disk usage and delete old files manually
2. **Backup Strategy**: File-based storage simplifies backups (copy directory tree)
3. **Monitoring**: Track `current.jsonl` size, rotation success rate, compression lag
4. **Migration Planning**: Estimate storage growth to plan time-series database migration
5. **Clock Synchronization**: Requires NTP on client and server for accurate UUIDv7 generation

## Implementation

1. Define protobuf schema for event message:
   - Map JSON schema fields to protobuf types (see Appendix A)
   - Use `google.protobuf.Timestamp` for timestamps
   - Use `google.protobuf.Struct` for nested record/metadata objects
   - Include both `client_timestamp` and `server_received_at`

2. Implement event storage layer:
   - Buffered channel for event ingestion (10,000 capacity)
   - Single writer task with periodic fsync (every 100 events or 1 second)
   - File rotation at hour boundaries with flock-based locking
   - Asynchronous compression with atomic rename (see Appendix D)

3. Implement startup recovery:
   - Always rotate `current.jsonl` on restart
   - Truncate incomplete JSON lines before rotation
   - Clean up `.gz.part` files and recompress from `.jsonl` source
   - Compress orphaned `.jsonl` files from past hours

4. Implement query interface:
   - Web UI for time-range selection
   - Basic filtering by rule_id, action, client metadata
   - Stream results from gzipped files
   - Download filtered results as JSONL

5. Implement client metadata validation:
   - Reject keys starting with `$` in SDK (see Appendix B)
   - Server-side enforcement of `$tk.*` namespace
   - Enforce size limits (50 pairs, 128 char keys, 1KB values, 64KB total)

6. Implement monitoring:
   - Log rotation events (timestamp, file size, compression duration)
   - Track current.jsonl size and fsync latency
   - Alert on rotation failures or compression lag

## Related Decisions

This ADR depends on the event schema capturing the rule state that triggered each event, enabling complete point-in-time auditability.

**Depends on:**
- **ADR-003: UUID Strategy** - Uses UUIDv7 for event_id generation with time-ordered uniqueness
- **ADR-014: Rule Expression Language** - Captures events when rules match during evaluation
- **ADR-018: Rule Lifecycle** - Requires rule snapshot in events to preserve audit trail when rules are modified or deleted

**Extended by:**
- **ADR-020: Client Metadata Namespace** - Defines the $tk.* prefix for system metadata in events

## Future Considerations

- **Time-series database migration**: TimescaleDB, InfluxDB, or QuasarDB for production scale
- **Columnar storage**: Parquet files for improved compression and query performance
- **Streaming ingestion**: Kafka/Kinesis integration for high-volume event pipelines
- **Event sampling**: Probabilistic sampling before storage to reduce volume
- **Dead letter queue**: Separate storage for events that fail validation
- **Cross-hour queries**: Index layer to enable efficient multi-file queries
- **Real-time aggregations**: Pre-computed rollups for common queries
- **Structured query language**: SQL-like interface for complex filtering
- **Cold storage archival**: Automatic migration to S3/GCS after retention threshold
- **Retention policy automation**: Configurable TTL with automatic cleanup and archival

## Appendix A: Event Schema Reference

Complete event schema example (JSON notation for documentation; actual implementation uses Protocol Buffers):

```json
{
  "event_id": "01936a3e-8f2a-7b3c-9d5e-123456789abc",
  "metadata": {
    // sdks collect default metadata whose keys all start with `$tk.`
    "$tk.api_type": "airflow",
    "$tk.api_version": "1.0.3",
    "$tk.client_ip": "10.0.1.5",
    "$tk.client_timestamp": "2025-01-15T14:32:11.123Z",
    "$tk.airflow_dag_id": "...",
    "$tk.airflow_task_id": "...",

    // custom keys specified by the user
    "custom_key": "custom_value"
  },

  "rule": {
    // full snapshot of rule at time of match
    "rule_id": "01936a3e-1234-7b3c-9d5e-abcdef123456",
    "name": "Invalid temperature check",
    "priority": 100,
    "action": "drop",
    "any": [...]
  },

  "action": "drop",
  "matched_field": ["customer", "age"],
  "matched_value": -1,

  "record": {
    "customer": {"name": "Acme Corp", "age": -1},
    "amount": 1500.00
  }
}
```

**Field Definitions**:

| Field | Type | Description |
|-------|------|-------------|
| `event_id` | UUIDv7 | Client-generated unique identifier for deduplication |
| `client_timestamp` | Timestamp | Timestamp when event occurred on client (UTC) |
| `client_ip` | String | IP address of client that generated event |
| `client_api_type` | String | SDK type (`"python"`, `"java"`, `"go"`, `"airflow"`, etc.) |
| `client_metadata` | Object | User-provided context plus system metadata |
| `rule` | Object | Full rule snapshot at time of match |
| `action` | String | Action taken (`"observe"`, `"drop"`, `"error"`) |
| `matched_field` | Array | Path to field that matched (e.g., `["customer", "age"]`) |
| `matched_value` | Any | Actual value that triggered the match |
| `record` | Object | Complete record that was evaluated |

**Design Rationale**:
- **Full rule snapshot**: Captures rule state even if later deleted or modified. Essential for compliance and debugging.
- **Complete record**: Enables root cause analysis without needing to replay pipelines or fetch external data.
- **Client-generated event_id**: UUIDv7 format enables client-side deduplication and provides natural time-ordering.
- **Client vs server timestamps**: `client_timestamp` reflects when event occurred, `server_received_at` (added by storage layer) reflects when event was ingested. Bucketing uses server time for consistency.

**Protobuf Implementation**:
- Strong type contracts prevent runtime errors
- Binary encoding reduces bandwidth
- Code generation ensures type-safe clients (Python, Java, Go)
- Schema evolution with backward compatibility
- Event schema fields map directly to protobuf message fields (see ADR-005)

## Appendix B: Client Metadata Namespace

Client metadata enables correlation with external systems (Airflow DAGs, Kubernetes pods, etc.):

**System metadata prefix** (`$tk.*`):
- Reserved for TrapperKeeper internal metadata
- Examples: `$tk.source`, `$tk.ingest_time`, `$tk.server_version`
- Client-side SDK rejects any custom keys starting with `$`
- Server overwrites any client-supplied `$tk.*` keys to prevent spoofing

**User metadata**:
- Arbitrary key-value pairs
- Max 50 pairs per sensor
- Max key length: 128 characters
- Max value length: 1024 characters
- Total metadata size: 64KB maximum
- UTF-8 encoding, no control characters

**Rationale**: Namespace separation prevents collision between user metadata and system-generated fields. Server enforcement prevents malicious clients from spoofing system metadata.

## Appendix C: File Storage Layout and Configuration

Directory structure for event storage:

```
/var/lib/trapperkeeper/
├── events/
│   ├── current.jsonl                    # Current hour (uncompressed)
│   ├── .rotate.lock                     # Advisory lock for rotation
│   ├── 2025/
│   │   └── 10/
│   │       └── 27/
│   │           ├── 2025-10-27-08-00-00.jsonl.gz
│   │           ├── 2025-10-27-09-00-00.jsonl.gz
│   │           └── 2025-10-27-10-00-00.jsonl.gz
```

**Configuration**:
- Base path: Configurable via `--data-dir` flag (e.g., `/var/lib/trapperkeeper`)
- Events directory: Always `{data-dir}/events/`
- Auto-create on startup if doesn't exist
- Current hour file: `events/current.jsonl` (uncompressed for tailing)
- Historical files: `events/YYYY/MM/DD/YYYY-MM-DD-HH-00-00.jsonl.gz`

**Timestamp indexing**:
- Use server receipt timestamp for entire batch (not individual client timestamps)
- Bucketing happens on server side, ensuring consistent file organization
- Clock drift warnings logged if client timestamp differs by >100ms

**Layout Rationale**:
- Year/month/day hierarchy enables efficient filesystem operations (deletion by date range)
- Hour-bucketed files balance query granularity with file count management
- Uncompressed `current.jsonl` allows real-time tailing with standard tools
- Advisory lock file (`.rotate.lock`) prevents concurrent rotation attempts
- Timestamp-based naming enables chronological sorting and glob patterns
- JSONL format is append-only, text-searchable, and tooling-friendly
- Compression reduces storage costs for historical data

**File Naming Convention**:
- Current hour: `current.jsonl` (symlink target, uncompressed)
- Historical: `YYYY-MM-DD-HH-00-00.jsonl.gz` (UTC timestamps)
- Temporary: `.jsonl.gz.part` (compression in progress, readers ignore)

## Appendix D: Atomic File Rotation and Concurrency

### File Rotation

File rotation uses atomic rename-then-compress pattern with advisory locking:

**Rotation process**:
1. Acquire exclusive lock: `flock` on `events/.rotate.lock`
2. Flush and fsync `current.jsonl`
3. Atomic rename: `current.jsonl` → `events/YYYY/MM/DD/YYYY-MM-DD-HH-00-00.jsonl`
4. Create new empty `current.jsonl`
5. Release lock immediately

**Asynchronous compression** (non-blocking):
1. Compress to temporary file: `.jsonl.gz.part`
2. Use gzip level 3-4 (balance speed vs ratio)
3. Atomic rename: `.gz.part` → `.gz`
4. Delete uncompressed `.jsonl` after verification

**Recovery on startup**:
- **Always rotate `current.jsonl` on service restart** (keeps recovery simple)
- Clean up incomplete JSON lines before rotation (truncate at last complete line)
- Remove orphaned `.gz.part` files and recompress from `.jsonl` source
- Compress any uncompressed `.jsonl` files from past hours

**Reader conventions**:
- Historical queries: Only read `.jsonl.gz` files
- Real-time monitoring: Tail `current.jsonl` (tolerate incomplete last line)
- Never read `.part` or intermediate files

**Rationale**: Atomic rename prevents partial file reads. Advisory locks prevent concurrent rotation. Asynchronous compression avoids blocking event ingestion. Startup recovery ensures consistency after crashes.

### Concurrent Event Writing

Single writer pattern ensures atomicity and simplifies concurrency model:

**Architecture**:
- Events sent to buffered queue (capacity: 10,000 events)
- Single writer task reads from queue and writes to file
- Periodic fsync: Every 100 events OR every 1 second (whichever comes first)
- Graceful shutdown: Flush buffer before exit
- No automatic flush: Explicit buffer management (see ADR-001)

**Benefits**:
- No file-level locking required (single writer)
- Predictable ordering within single server instance
- Simple error handling (channel backpressure)
- Fsync batching amortizes disk sync overhead

**Backpressure handling**:
- Channel blocks when full
- gRPC call blocks until event written
- Client SDK buffer provides additional buffering
- Sampling reduces load if needed

**Rationale**: Single writer eliminates race conditions. Buffered channel decouples HTTP handlers from disk I/O. Periodic fsync balances durability vs performance.

## Appendix E: Query Interface and Retention Policy

### Query Interface

**MVP capabilities**:
- Time-range filtering (required)
- Basic field filtering (rule_id, action, client metadata)
- Web UI displays results in table format
- Download filtered results as JSONL

**Future enhancements** (out of scope for MVP):
- Full-text search across record fields
- Aggregations (count by rule_id, action distribution)
- Real-time tail mode in Web UI
- Structured query language for complex filters

**Rationale**: MVP focuses on core audit and debugging use cases. Simple grep-based filtering sufficient for hour-bucketed files. Full-text search deferred until storage backend migrated to time-series database.

### Retention Policy

**MVP approach**:
- Manual deletion only
- Operator deletes old directories/files via filesystem
- No automatic cleanup or TTL enforcement

**Future considerations**:
- Configurable retention period (global or per-rule)
- Automatic deletion of files older than retention window
- Archive to cold storage before deletion
- Retention policy enforcement in query layer

**Rationale**: Explicit deletion prevents accidental data loss. Simple filesystem operations sufficient for MVP. Automatic retention requires policy management UI.
