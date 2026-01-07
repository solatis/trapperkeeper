# SensorAPI Handlers

## Overview

gRPC service handlers for tk-sensor-api enabling sensors to synchronize rules (SyncRules) and report events (ReportEvents). Thin orchestration layer delegating business logic to internal/rules for evaluation and internal/core/db for persistence. Stateless protocol design enables horizontal scaling without session replication.

## Architecture

```
SyncRules Flow:
  Request (tags, if-none-match) -> Auth (via interceptor)
                                -> Query rules by tenant_id from database
                                -> Compute ETAG as SHA256(sorted rule_ids + created_at)
                                -> Compare with if-none-match (bandwidth optimization)
                                -> Return rules or empty response (304-like)

ReportEvents Flow:
  Request (events[], sensor_id) -> Auth (via interceptor)
                                -> Validate batch size (<= 1000)
                                -> Determine JSONL filename at request start
                                -> Per-event processing:
                                    - Validate event structure
                                    - Insert to database (own transaction)
                                    - Write to JSONL (best-effort, with mutex)
                                    - Collect per-event result
                                -> Return per-event status (ACCEPTED/REJECTED)
```

SensorAPIService has no business logic. Rule evaluation delegated to internal/rules engine. Database queries use internal/core/db. Handlers coordinate and map errors to gRPC status codes.

## Design Decisions

**ETAG as SHA256(rule_ids + timestamps)**: Rule sync happens frequently (every 30s default). Sending full rules wastes bandwidth when unchanged. SHA256 of concatenated identifiers is deterministic and collision-resistant. Server computes on-demand without caching layer. Same rule set always produces same ETAG (content-addressable).

**gRPC unary RPCs over streaming**: Sensors batch events locally, flush periodically. Unary RPC simpler than bidirectional stream -- server stateless with no stream state to manage. Aligns with ephemeral sensor principle. Achieves same throughput as streaming with simpler implementation.

**Per-event status in ReportEvents response**: Sensors batch heterogeneous events from multiple sources. Partial batch failure common (one malformed event shouldn't discard 99 valid events). Per-event status enables targeted retry without re-sending accepted events. Alternative (fail entire batch) wastes bandwidth and creates thundering herd on retry.

**Per-event DB transactions in ReportEvents**: Each event inserted in own transaction (not atomic batch). Enables per-event success/failure status. Partial batch failure returns mixed results (some ACCEPTED, some REJECTED). Client can retry only failed events. Simplifies error handling at cost of transaction overhead.

**1000 event max batch size**: Balances throughput (~1MB typical payload) vs transaction time. Prevents single request from exhausting connection pool or causing timeout. Enforced at API layer before processing. Configurable via TK_MAX_BATCH_SIZE environment variable.

**JSONL output path $TK_DATA_DIR/events/**: Daily rotation files (YYYY-MM-DD.jsonl) provide debugging visibility into event stream. Auto-create directory on first ReportEvents to reduce operational friction. Per-file mutex protects concurrent writes to same daily file. JSONL writes are best-effort (success not required for event acceptance).

**Per-file mutex for JSONL writes**: Concurrent ReportEvents calls write to same daily file. Uncoordinated writes corrupt JSONL output. Mutex per filename (map[string]*sync.Mutex) ensures serialized writes. Acceptable lock contention for debugging use case. Mutex map grows by ~1 entry/day (365 entries/year, negligible memory).

**JSONL filename determined at request start**: Batch processing spanning midnight could split events across files. Simpler to use single timestamp per batch. All events in batch written to file of request arrival time (time.Now() at handler entry).

**Auto-create events directory**: First ReportEvents may occur before manual directory setup. os.MkdirAll with 0755 permissions reduces operational friction. Parent directory ($TK_DATA_DIR) must exist (validated at service startup).

## Invariants

1. **ETAG is content-addressable**: Same rule set always produces same ETAG. No timestamps or random components in ETAG computation. SHA256(sorted rule_ids + created_at) is deterministic. Enables bandwidth-efficient sync via if-none-match comparison.

2. **JSONL may contain events not in database**: JSONL writes are best-effort per-event and may succeed even if individual DB insert fails. JSONL may also be missing events that succeeded in DB (if JSONL write failed). Do not rely on JSONL as authoritative source. Database is source of truth. JSONL is for debugging only.

3. **Max batch size enforced at API layer**: Batches exceeding 1000 events rejected with INVALID_ARGUMENT before any processing. Prevents transaction timeouts, memory exhaustion, and connection pool starvation. No partial processing occurs for oversized batches.

4. **JSONL daily file determined at request start**: All events in a single ReportEvents batch are written to the same JSONL file, determined by time.Now() at handler entry. Batch processing spanning midnight does not split events across files. Simplifies debugging by keeping batches together.

5. **Per-event transactions, not atomic batch**: Each event in ReportEvents is inserted in its own DB transaction. Partial batch failure is possible and expected. Per-event status in response reflects individual insert outcomes. Clients must handle mixed success/failure results.

6. **Handlers assume authenticated context**: Auth interceptor executes before handlers. TenantIDFromContext(ctx) always returns non-empty string when handler invoked. Handlers do not validate authentication (already done by interceptor).

## Tradeoffs

| Choice                 | Benefit                                    | Cost                                                 |
| ---------------------- | ------------------------------------------ | ---------------------------------------------------- |
| Unary over streaming   | Simpler implementation, easier debugging   | Cannot push rule updates to sensors (polling needed) |
| Per-event transactions | Partial batch success, targeted retry      | Higher transaction overhead vs single batch txn      |
| JSONL debugging output | Visibility into event stream for debugging | Mutex contention on daily file, disk I/O overhead    |
| Max batch size 1000    | Prevents resource exhaustion               | Large event volumes need multiple requests           |
