---
doc_type: hub
status: active
primary_category: architecture
consolidated_spokes:
  - event-schema-storage.md
  - identifiers-uuidv7.md
  - timestamps.md
  - metadata-namespace.md
tags:
  - events
  - uuidv7
  - timestamps
  - metadata
---

# Data Architecture

## Context

TrapperKeeper's data model must support schema-agnostic event storage, preserving full audit trails while maintaining query performance. The system handles diverse data formats from sensors (compressed JSON, CSV, Parquet waveforms) without requiring pre-registration of schemas.

Traditional data warehouses require schema-on-write with predefined tables and columns. Time-series databases optimize for append-only workloads but struggle with arbitrary JSON. Document databases provide flexibility but sacrifice query performance for analytical workloads. TrapperKeeper needs a middle ground: flexible event storage with structured metadata enabling efficient queries.

This hub consolidates data architecture decisions establishing event schema, identifier strategy (UUIDv7), timestamp handling across boundaries, and metadata namespace management.

## Decision

We implement **JSONL file-based event storage** with UUIDv7 identifiers, multi-layer timestamp handling, and reserved metadata namespace. Events are stored as append-only JSON Lines files preserving full audit trails, with UUIDv7 providing natural time-ordering and efficient indexing.

This document serves as the data hub providing strategic overview with cross-references to detailed implementation documents for event storage, identifiers, timestamps, and metadata.

### Event Storage Model

Events stored as JSONL (JSON Lines) files in filesystem with full audit trail preservation:

**Storage Format**:

- One event per line (newline-delimited JSON)
- Append-only writes for maximum throughput
- No updates or deletes (soft deletes via marker records)
- Compressed rotation for long-term storage
- Database indexes enable fast queries

**Event Structure**:

```json
{
  "event_id": "018c9f8e-1234-7abc-9def-0123456789ab",
  "client_timestamp": "2025-10-29T10:00:00.000Z",
  "server_received_at": "2025-10-29T10:00:01.123Z",
  "payload": { ... },
  "metadata": { "sensor_id": "temp-01", "$tk.api_version": "0.1.0" },
  "matched_rules": [ ... ],
  "rule_snapshot": { ... },
  "record_snapshot": { ... }
}
```

**Key Principles**:

- Schema-agnostic: `payload` field accepts arbitrary JSON
- Full audit trail: Rule snapshot and record snapshot preserved
- Time-ordered: UUIDv7 `event_id` provides natural sorting
- Query-optimized: Database indexes on event_id, sensor_id, timestamps

**Cross-References**:

- Event Schema and Storage: Complete storage specification
- Identifiers (UUIDv7): Event ID generation and properties
- Timestamp Representation: Timestamp field handling

**Example**:

Event file `/var/lib/trapperkeeper/events/2025-10-29.jsonl`:

```
{"event_id":"018c9f8e-1234-7abc-9def-0123456789ab","payload":{...}}
{"event_id":"018c9f8e-5678-7abc-9def-0123456789ab","payload":{...}}
{"event_id":"018c9f8e-9abc-7abc-9def-0123456789ab","payload":{...}}
```

Each line is independent JSON record enabling streaming processing and partial reads.

### UUIDv7 Identifier Strategy

All system identifiers use UUIDv7 format providing time-ordered, globally unique IDs:

**UUIDv7 Properties**:

- 48-bit timestamp (millisecond precision)
- 74-bit random component (collision resistance)
- Lexicographically sortable (timestamp-ordered)
- Globally unique without coordination
- Compatible with UUID standards

**Identifier Types**:

- `event_id`: Event identifier (primary key)
- `rule_id`: Rule identifier
- `user_id`: User identifier
- `sensor_id`: Sensor identifier (when using UUID format)
- `tenant_id`: Tenant identifier (multi-tenancy preparation)

**Benefits**:

- Natural time-ordering: UUIDs sort chronologically without explicit timestamp field
- Database-friendly: Efficient B-tree indexing (no random page updates)
- No coordination: Clients and servers generate IDs independently
- Collision-resistant: 74 random bits provide strong uniqueness guarantees

**Rationale**: UUIDv4 (random) causes database index fragmentation. UUIDv1 (timestamp + MAC) leaks hardware information. UUIDv7 provides best of both: time-ordering for database performance plus strong randomness for security.

**Cross-References**:

- Identifiers (UUIDv7): Complete specification and generation algorithm
- Event Schema and Storage: UUIDv7 usage in event IDs
- Database Backend: Index optimization for UUIDv7

**Example Generation**:

```go
import "github.com/google/uuid"

// Generate UUIDv7 (time-ordered)
eventID := uuid.Must(uuid.NewV7())
// -> 018c9f8e-1234-7abc-9def-0123456789ab

// Extract timestamp
timestamp := eventID.Time()
// -> 2025-10-29T10:00:00.123Z
```

### Multi-Layer Timestamp Handling

Timestamps represented differently at each architectural boundary optimized for that layer:

**Layer 1 - Protocol Boundary** (`google.protobuf.Timestamp`):

- Wire format for gRPC communication
- Nanosecond precision
- Language-agnostic serialization
- Used in protobuf message definitions

**Layer 2 - Application Layer** (`time.Time`):

- Go application code standard
- encoding/json and database/sql integration
- Type-safe operations
- UTC timezone enforced

**Layer 3 - Database Layer** (TIMESTAMP types):

- PostgreSQL: Microsecond precision
- SQLite: Nanosecond precision (TEXT ISO8601)
- Varies by backend
- Query-optimized storage

**Conversion Points**:

Explicit conversions at boundary crossings:

- gRPC -> Go: `timestamppb.Timestamp.AsTime()`
- Go -> gRPC: `timestamppb.New(t)`
- Go -> Database: database/sql automatic mapping
- Database -> Go: database/sql automatic mapping

**Key Principles**:

- UTC everywhere: No local timezone assumptions
- Explicit conversions: Clear boundary crossing points
- Type safety: Compiler prevents format mixing
- Standard tooling: Leverage existing ecosystem

**Cross-References**:

- Timestamp Representation: Complete conversion specifications
- Event Schema and Storage: Timestamp field usage
- Database Backend: Database TIMESTAMP type configuration

**Example Conversion**:

```go
// Protocol -> Application
protoTS := event.ClientTimestamp
chronoTS := protoTS.AsTime()

// Application -> Database
_, err := db.Exec("INSERT INTO events (client_timestamp) VALUES (?)", chronoTS)
if err != nil {
    return err
}

// Database -> Application (automatic)
var event Event
err = db.QueryRow("SELECT * FROM events WHERE id = ?", id).Scan(&event.ID, &event.ClientTimestamp)
// event.ClientTimestamp is time.Time
```

### Client Metadata Namespace

Reserved `$tk.*` prefix for system metadata prevents collision with user-provided metadata:

**System Metadata** (auto-collected):

- `$tk.api_type`: SDK type (`"python"`, `"java"`, `"airflow"`)
- `$tk.api_version`: TrapperKeeper SDK version
- `$tk.client_ip`: IP address of sensor
- `$tk.client_timestamp`: Sensor-side event timestamp
- `$tk.server_received_at`: Server ingestion timestamp
- `$tk.server_version`: TrapperKeeper server version

**User Metadata** (custom):

- Any keys NOT starting with `$`
- Max 64 key-value pairs per sensor
- Max 128 characters per key (UTF-8)
- Max 1024 characters per value (UTF-8)
- Max 64KB total metadata size

**Enforcement**:

Client-side: SDKs reject keys starting with `$`
Server-side: API server strips client-supplied `$tk.*` keys and overwrites with correct values

**Rationale**: Prefix-based namespace separation prevents collision. Server enforcement prevents spoofing of SDK version or ingestion time. The `$` prefix convention aligns with templating languages and special identifiers.

**Cross-References**:

- Client Metadata Namespace: Complete specification and validation rules
- SDK Model Section 7: Metadata collection patterns
- Event Schema and Storage: Metadata field validation

**Example Metadata**:

```json
{
  "metadata": {
    "sensor_id": "temp-sensor-01",
    "team": "data-platform",
    "environment": "production",
    "$tk.api_type": "python",
    "$tk.api_version": "0.1.0",
    "$tk.client_ip": "192.168.1.100"
  }
}
```

User provided: `sensor_id`, `team`, `environment`
System added: `$tk.*` fields

## Consequences

**Benefits:**

- Schema flexibility: Arbitrary JSON payloads without pre-registration
- Full audit trail: Complete event history with rule and record snapshots
- Query performance: Database indexes enable fast lookups despite JSONL storage
- Natural ordering: UUIDv7 provides chronological sorting
- Type safety: Multi-layer timestamps prevent format mixing
- Namespace protection: Reserved prefix prevents metadata collision
- Collision resistance: UUIDv7 randomness eliminates coordination

**Trade-offs:**

- Storage overhead: Full audit trail increases storage 2-3x vs minimal events
- Precision loss: Database timestamps lose nanoseconds (PostgreSQL)
- Multiple formats: Timestamps represented differently at each layer
- Reserved namespace: Users cannot use `$` prefix for custom metadata
- JSONL limitations: No in-place updates (append-only only)

**Operational Implications:**

- Event files rotate daily for manageable file sizes
- Compression applied to rotated files reducing storage 70-80%
- Database indexes updated on writes (minor performance impact)
- Clock synchronization via NTP required for accurate timestamps
- Backup strategy must handle both JSONL files and database

## Related Documents

**Consolidated Spokes** (this hub consolidates):

- Event Schema and Storage: Maps to event storage model section, provides complete JSONL specification
- Identifiers (UUIDv7): Maps to UUIDv7 strategy section, provides generation algorithm
- Timestamp Representation: Maps to multi-layer timestamps section, provides conversion utilities
- Client Metadata Namespace: Maps to metadata namespace section, provides validation rules

**Dependencies** (foundational documents):

- Principles Architecture: Establishes schema-agnostic principle informing event storage
- Database Backend: Defines database layer for event indexing

**References** (related hubs/documents):

- Architecture Hub: API service and SDK model using event schema
- Operations Hub: Database migrations for event schema tables

**Extended by**:

- Rule Lifecycle: Rule snapshots stored in event audit trail
- Field Path Resolution: Field path evaluation on event payloads
- Type System and Coercion: Type handling for event payload values
