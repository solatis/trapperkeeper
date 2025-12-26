---
doc_type: spoke
status: active
date_created: 2025-11-07
primary_category: database
hub_document: /Users/lmergen/git/trapperkeeper/doc/03-data/README.md
tags:
  - jsonl
  - audit-trail
  - schema-agnostic
---

# Event Schema and Storage

## Context

TrapperKeeper events must store arbitrary JSON payloads from sensors without requiring pre-registered schemas, while preserving full audit trails for compliance and debugging. Storage must support high-throughput writes (10,000 events/second) and efficient queries for operational dashboards and investigations.

The event storage architecture uses JSONL (JSON Lines) files for append-only writes with database indexes for fast queries. Each event preserves the full context: original payload, matched rules with snapshots, and execution metadata.

**Hub Document**: This document is part of the Data Hub. See Data Architecture for strategic overview of event storage, identifiers, timestamps, and metadata within TrapperKeeper's data model.

## JSONL File Format

Events stored as newline-delimited JSON (JSONL) in append-only files.

### File Organization

```
/var/lib/trapperkeeper/events/
├── 2025-10-29.jsonl           # Active file (today)
├── 2025-10-28.jsonl.gz        # Compressed (yesterday)
├── 2025-10-27.jsonl.gz        # Compressed (older)
└── .processing/               # Temporary processing files
    └── batch-abc123.jsonl     # Batch in progress
```

**File Rotation**:

- New file created daily at midnight UTC
- Previous day's file compressed with gzip (~70-80% reduction)
- Rotation handled by background task
- Active file never compressed (enables fast appends)

**File Naming**:

- Format: `YYYY-MM-DD.jsonl` (date-based)
- Compressed: `YYYY-MM-DD.jsonl.gz`
- Temporary: `.processing/batch-{uuid}.jsonl`

### Line Format

Each line is complete, independent JSON object:

```json
{
  "event_id": "018c9f8e-1234-7abc-9def-0123456789ab",
  "client_timestamp": "2025-10-29T10:00:00.000Z",
  "server_received_at": "2025-10-29T10:00:01.123Z",
  "payload": { "sensor_id": "temp-01", "temperature": 95.5, "humidity": 45.2 },
  "metadata": { "team": "data-platform", "$tk.api_version": "0.1.0" },
  "matched_rules": [
    {
      "rule_id": "018c9f8e-aaaa-7abc-9def-0123456789ab",
      "action": "observe",
      "severity": "critical"
    }
  ],
  "rule_snapshot": { "expression": "$.temperature > 80", "state": "active" },
  "record_snapshot": { "temperature": 95.5 }
}
```

**Benefits**:

- Streaming writes: Append single line without reading entire file
- Partial reads: Process subset of events without loading all
- Fault tolerance: Corrupted line doesn't affect others
- Standard tooling: Compatible with `jq`, `grep`, Unix pipelines

**Example Processing**:

```bash
# Count events with temperature > 90
cat 2025-10-29.jsonl | jq 'select(.payload.temperature > 90)' | wc -l

# Extract specific sensor events
grep '"sensor_id":"temp-01"' 2025-10-29.jsonl | jq .

# Compress old files
gzip events/2025-10-28.jsonl
```

**Cross-References**:

- Data Architecture Section 1: Event storage format overview
- Batch Processing and Vectorization: Streaming processing patterns

## Event Schema Structure

Complete event schema with required and optional fields.

### Core Fields

**Required Fields**:

```json
{
  "event_id": "018c9f8e-1234-7abc-9def-0123456789ab",
  "client_timestamp": "2025-10-29T10:00:00.000Z",
  "server_received_at": "2025-10-29T10:00:01.123Z",
  "payload": {
    /* arbitrary JSON */
  }
}
```

- `event_id`: UUIDv7 identifier (time-ordered, globally unique)
- `client_timestamp`: Sensor-side timestamp (ISO8601 UTC)
- `server_received_at`: Server ingestion timestamp (ISO8601 UTC)
- `payload`: Arbitrary JSON object from sensor (schema-agnostic)

**Optional Fields**:

```json
{
  "metadata": {
    "sensor_id": "temp-01",
    "team": "data-platform",
    "$tk.api_version": "0.1.0",
    "$tk.api_type": "python"
  },
  "matched_rules": [
    {
      "rule_id": "018c9f8e-aaaa-7abc-9def-0123456789ab",
      "action": "observe",
      "severity": "critical"
    }
  ],
  "rule_snapshot": {
    "rule_id": "018c9f8e-aaaa-7abc-9def-0123456789ab",
    "name": "High Temperature",
    "expression": "$.temperature > 80",
    "state": "active",
    "created_at": "2025-10-01T00:00:00Z"
  },
  "record_snapshot": {
    "temperature": 95.5,
    "humidity": 45.2
  }
}
```

- `metadata`: User and system metadata (see Client Metadata Namespace)
- `matched_rules`: Array of rules that matched this event
- `rule_snapshot`: Full rule definition at evaluation time
- `record_snapshot`: Subset of payload that caused match

**Cross-References**:

- Identifiers (UUIDv7): Event ID generation
- Timestamp Representation: Timestamp field formats
- Client Metadata Namespace: Metadata validation rules

### Payload Field

Schema-agnostic JSON accepting arbitrary structure:

**Valid Payloads**:

```json
// Flat object
{"sensor_id": "temp-01", "value": 95.5}

// Nested objects
{"device": {"id": "temp-01", "location": {"building": "A", "floor": 3}}, "reading": 95.5}

// Arrays
{"readings": [{"time": "10:00", "value": 95.5}, {"time": "10:01", "value": 96.0}]}

// Mixed types
{"id": "temp-01", "active": true, "readings": [95.5, 96.0], "metadata": {"tags": ["urgent"]}}
```

**Constraints**:

- Must be valid JSON object (not primitive or array at root)
- Size limit: 1MB per event payload
- Encoding: UTF-8 only
- No null at root level

**Validation**: Payload validated for JSON structure only, no schema enforcement.

**Cross-References**:

- Principles Architecture Principle #1: Schema-agnostic architecture
- Field Path Resolution: Field path evaluation on arbitrary payloads

## Audit Trail Preservation

Events preserve complete context for compliance and debugging.

### Rule Snapshot

Full rule definition at time of evaluation:

```json
{
  "rule_snapshot": {
    "rule_id": "018c9f8e-aaaa-7abc-9def-0123456789ab",
    "name": "High Temperature",
    "expression": "$.temperature > 80",
    "action": "observe",
    "state": "active",
    "on_missing_field": "skip",
    "sample_rate": 1.0,
    "created_at": "2025-10-01T00:00:00Z",
    "modified_at": "2025-10-15T12:00:00Z"
  }
}
```

**Rationale**: Rules may be modified or deleted after event creation. Snapshot preserves original rule definition for audit trail.

**Use Cases**:

- Compliance audits: Prove which rule triggered action
- Debugging: Understand why event was flagged
- Historical analysis: Compare rule evolution over time

### Record Snapshot

Subset of payload fields that matched rule conditions:

```json
{
  "record_snapshot": {
    "temperature": 95.5,
    "humidity": 45.2,
    "sensor_id": "temp-01"
  }
}
```

**Extraction Logic**:

- Include all fields referenced in rule expression
- Include array elements matched by wildcard conditions
- Preserve nested structure for matched fields
- Limit: 100KB per record snapshot

**Rationale**: Full payload may be large (1MB). Record snapshot provides focused view of fields causing match.

**Example**:

Rule: `$.devices[*].temperature > 80`

Payload:

```json
{
  "devices": [
    { "id": "temp-01", "temperature": 95.5, "location": "A" },
    { "id": "temp-02", "temperature": 70.0, "location": "B" },
    { "id": "temp-03", "temperature": 85.0, "location": "C" }
  ]
}
```

Record Snapshot:

```json
{
  "devices": [
    { "id": "temp-01", "temperature": 95.5 },
    { "id": "temp-03", "temperature": 85.0 }
  ]
}
```

Only matched array elements included.

**Cross-References**:

- Rule Lifecycle: Rule modification and deletion
- Field Path Resolution: Wildcard matching logic

## Database Indexing

Database indexes enable fast queries despite JSONL storage.

### Event Table Schema

```sql
CREATE TABLE events (
  event_id UUID PRIMARY KEY,
  client_timestamp TIMESTAMP NOT NULL,
  server_received_at TIMESTAMP NOT NULL,
  file_path TEXT NOT NULL,           -- Path to JSONL file
  file_offset BIGINT NOT NULL,       -- Byte offset in file
  payload_hash BINARY(32),           -- SHA256 of payload
  matched_rule_count INTEGER,
  INDEX idx_client_timestamp (client_timestamp),
  INDEX idx_server_received_at (server_received_at),
  INDEX idx_file_path (file_path)
);
```

**Query Strategy**:

1. Query database index for matching event IDs
2. Retrieve `file_path` and `file_offset` for each ID
3. Seek to offset in JSONL file and read line
4. Parse JSON and return to client

**Benefits**:

- Fast queries: Database indexes enable millisecond lookups
- Storage efficiency: Full event data in compressed JSONL files
- Append-only: Database inserts only (no updates)
- Simple backup: JSONL files contain complete data

**Example Query**:

```go
import (
    "database/sql"
    "encoding/json"
)

// Query database for recent events
rows, err := db.Query(`
    SELECT event_id, file_path, file_offset
    FROM events
    WHERE client_timestamp > ?
    ORDER BY client_timestamp DESC
    LIMIT 100`,
    sinceTimestamp,
)
if err != nil {
    return err
}
defer rows.Close()

// Read full events from JSONL files
for rows.Next() {
    var eventID, filePath string
    var fileOffset int64
    rows.Scan(&eventID, &filePath, &fileOffset)

    line := readLineAtOffset(filePath, fileOffset)
    var fullEvent Event
    json.Unmarshal([]byte(line), &fullEvent)
    // ...
}
```

**Cross-References**:

- Database Backend: Index optimization strategies
- Identifiers (UUIDv7): UUIDv7 B-tree index performance

## Write Performance Optimization

Batch writes and async I/O enable high throughput.

### Batch Insertion

```go
// Buffer events in memory
batch := make([]Event, 0, 1000)

for event := range incomingEvents {
    batch = append(batch, event)

    if len(batch) >= 1000 {
        writeBatch(file, db, batch)
        batch = batch[:0] // Reset slice
    }
}

// Flush remaining events
if len(batch) > 0 {
    writeBatch(file, db, batch)
}
```

**Write Strategy**:

1. Accumulate 1000 events in memory
2. Append all to JSONL file (single `write_all()` call)
3. Insert index records in database (single prepared statement)
4. Commit transaction
5. Clear buffer and repeat

**Performance**:

- Throughput: 10,000 events/second per instance
- Latency: p99 < 100ms for batch write
- Buffer: 1000 events optimal (configurable)

**Cross-References**:

- API Service Architecture Section 8: Performance characteristics
- Batch Processing and Vectorization: Vectorized operations

## Query Patterns

Common query patterns for operational dashboards.

### Time Range Queries

```sql
-- Events in last hour
SELECT event_id, client_timestamp, payload
FROM events
WHERE client_timestamp > NOW() - INTERVAL '1 hour'
ORDER BY client_timestamp DESC;
```

### Rule Match Queries

```sql
-- Events matching specific rule
SELECT event_id, client_timestamp, matched_rules
FROM events
WHERE matched_rule_count > 0
  AND event_id IN (
    SELECT event_id FROM event_rule_matches WHERE rule_id = $1
  )
ORDER BY client_timestamp DESC;
```

### Sensor Queries

```sql
-- Events from specific sensor
SELECT event_id, client_timestamp
FROM events
WHERE metadata->>'sensor_id' = $1
ORDER BY client_timestamp DESC
LIMIT 100;
```

**Cross-References**:

- Database Backend: Query optimization strategies

## Related Documents

**Dependencies** (read these first):

- Data Architecture: Event storage strategic overview
- Principles Architecture: Schema-agnostic principle

**Related Spokes** (siblings in this hub):

- Identifiers (UUIDv7): Event ID generation
- Timestamp Representation: Timestamp field handling
- Client Metadata Namespace: Metadata validation rules

**Extended by**:

- Rule Lifecycle: Rule snapshot storage
- Field Path Resolution: Payload field extraction
