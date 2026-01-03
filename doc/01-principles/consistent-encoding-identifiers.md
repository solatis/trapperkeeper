---
doc_type: spoke
hub_document: doc/01-principles/README.md
status: active
primary_category: architecture
title: Consistent Encoding and Identifiers
tags:
  - utf-8
  - uuidv7
  - identifiers
  - encoding
  - time-series
---

# Consistent Encoding and Identifiers

## Core Principle

**Use UTF-8 everywhere and UUIDv7 for all identifiers.**

Establish consistent text encoding and identifier format across all system
components. Eliminate entire classes of encoding bugs and identifier collisions.

## Motivation

### UTF-8 Encoding

Inconsistent character encoding causes:

- Mojibake (corrupted text display)
- Data corruption on round-trip (read -> modify -> write)
- Security vulnerabilities (encoding-based injection attacks)
- Cross-language interop issues (Python str vs bytes, Go string vs []byte)

**Decision:** UTF-8 everywhere eliminates these issues. No Latin-1, no ASCII, no
UTF-16. UTF-8 is universal, backward-compatible with ASCII, and has excellent
tooling support.

### UUIDv7 Identifiers

Traditional identifier choices have tradeoffs:

**Auto-increment integers:**

- Not globally unique (collision risk in distributed systems)
- Sequential leaks information (e.g., customer count)
- Requires coordination (database sequence)

**UUIDv4 (random):**

- Globally unique
- Not sortable (random order)
- Poor database index performance (random insertion causes page splits)

**UUIDv7 (time-ordered):**

- Globally unique
- Sortable (time-ordered, newest IDs sort last)
- Efficient database indexing (append-mostly insertion pattern)
- Contains timestamp (useful for time-series data)

**Decision:** UUIDv7 balances global uniqueness, sortability, and database
performance. Critical for time-series event storage.

## UTF-8 Encoding Rules

### All String Storage

Database columns storing text use UTF-8:

```sql
CREATE TABLE rules (
  rule_id TEXT PRIMARY KEY,     -- UTF-8 encoded
  name TEXT NOT NULL,            -- UTF-8 encoded
  expression TEXT NOT NULL       -- UTF-8 encoded
);
```

SQLite stores TEXT as UTF-8 by default. PostgreSQL requires explicit `ENCODING
'UTF8'` at database creation (enforced in initialization scripts).

### User-Generated Content

Web UI inputs accept UTF-8:

- Rule names: UTF-8 (e.g., "Detección de fraude", "使用者驗證")
- Rule expressions: UTF-8 string literals (e.g., `user.name == "José"`)
- Team names: UTF-8
- User display names: UTF-8

**Validation:** No encoding validation required. All UTF-8 sequences accepted
(including emoji, CJK characters, combining diacritics).

### Client/Sensor Data

Sensors send UTF-8 encoded event data:

- JSON payloads: UTF-8 encoded (JSON spec requires UTF-8)
- String field values: UTF-8
- Field names: UTF-8 (though ASCII recommended for interop)

**SDK Responsibility:** SDKs serialize event data as UTF-8 before sending to
server.

### Language-Specific Handling

Different languages have different UTF-8 handling:

**Python:**

- `str` type is Unicode (UTF-8 compatible)
- Encode to bytes: `s.encode('utf-8')`
- Decode from bytes: `b.decode('utf-8')`

**Go:**

- `string` type is UTF-8 by default
- Byte slices: `[]byte(s)` converts string to UTF-8 bytes
- No explicit encoding/decoding needed for most operations

**JavaScript:**

- Strings are UTF-16 internally, but JSON serialization produces UTF-8
- `TextEncoder`/`TextDecoder` for explicit UTF-8 conversion

**Interop Rule:** At system boundaries (HTTP, gRPC, database), always use UTF-8.
Language-internal representations can differ.

## UUIDv7 Identifier Rules

### All Entities Use UUIDv7

Every entity in the system has a UUIDv7 identifier:

- Tenants: `tenant_id TEXT PRIMARY KEY`
- Teams: `team_id TEXT PRIMARY KEY`
- Users: `user_id TEXT PRIMARY KEY`
- Rules: `rule_id TEXT PRIMARY KEY`
- API Keys: `api_key_id TEXT PRIMARY KEY`
- Events: `event_id TEXT PRIMARY KEY`

No auto-increment IDs, no composite keys (except junction tables).

### UUIDv7 Format

UUIDv7 structure (RFC 4122 variant):

```
0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                         unix_ts_ms                            |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|         unix_ts_ms            |  ver  |       rand_a          |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|var|                       rand_b                              |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                           rand_b                              |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

- **unix_ts_ms** (48 bits): Unix timestamp in milliseconds
- **ver** (4 bits): Version field (0111 for UUIDv7)
- **rand_a** (12 bits): Random bits
- **var** (2 bits): Variant field (10 for RFC 4122)
- **rand_b** (62 bits): Random bits

**Key Properties:**

- Time-ordered: UUIDs sort chronologically (newest last)
- Globally unique: 74 bits of randomness (collision probability negligible)
- Fixed length: 36 characters in canonical form
  (`018d3f72-1234-7abc-9def-0123456789ab`)

### Time-Ordered Benefits

UUIDv7 sorts chronologically, enabling efficient queries:

**Event range queries:**

```sql
-- Get events from last hour (efficient index scan)
SELECT * FROM events
WHERE event_id >= uuidv7_from_timestamp(NOW() - INTERVAL '1 hour')
ORDER BY event_id;
```

**Pagination:**

```sql
-- Paginate events (keyset pagination, no OFFSET overhead)
SELECT * FROM events
WHERE event_id > :last_seen_id
ORDER BY event_id
LIMIT 100;
```

**Index Performance:**

Time-ordered insertion is append-mostly, reducing B-tree page splits and index
fragmentation.

### NTP Synchronization Requirement

UUIDv7 time-ordering assumes synchronized clocks:

**Requirements:**

- NTP daemon running on all servers (ntpd, chrony, systemd-timesyncd)
- Clock drift within +/-100ms (UUIDv7 timestamp precision is 1ms)
- Periodic NTP sync (every 5 minutes recommended)

**Clock Skew Handling:**

- Accept client-generated UUIDs as-is (no server-side validation of timestamp)
- Out-of-order UUIDs (due to clock skew) are valid and stored
- Query results may be slightly non-chronological if clocks skewed

**Rationale:** Rejecting UUIDs due to clock skew creates denial-of-service risk.
Accept all UUIDs; tolerate minor ordering anomalies.

### Client vs Server Generation

**Client-Generated (Sensors, Web UI):**

- SDKs generate UUIDv7 for event IDs
- Web UI generates UUIDv7 for entity creation (rules, teams)
- Reduces server load (no centralized ID generation bottleneck)
- Requires client-side UUIDv7 library

**Server-Generated (API Server):**

- Server generates UUIDv7 for API keys (security-sensitive, server controls
  generation)
- Server generates UUIDv7 for user accounts (created via server-side
  authentication flow)

**General Rule:** Client generates IDs when client initiates creation. Server
generates IDs when server controls lifecycle.

## Benefits

1. **Encoding Consistency**: No mojibake, no corruption, no cross-language
   encoding bugs
2. **Global Uniqueness**: No ID collisions across distributed components
3. **Time-Series Efficiency**: UUIDv7 sorting aligns with event time-ordering
4. **Index Performance**: Append-mostly insertion pattern reduces database index
   overhead
5. **Simplified Queries**: Range queries use ID field directly (no separate
   timestamp column needed)
6. **Cross-Language Interop**: UTF-8 and UUID are universally supported

## Tradeoffs

1. **Clock Dependency**: UUIDv7 requires NTP synchronization (operational
   overhead)
2. **Storage Overhead**: UUIDs (36 bytes) larger than integers (8 bytes)
3. **Human Readability**: UUIDs harder to read/type than sequential integers
4. **Timestamp Precision**: 1ms precision may cause ordering ambiguity for
   high-frequency events
5. **Client Library Requirement**: SDKs must include UUIDv7 generation (not all
   languages have built-in support)

## Implementation

### UUIDv7 Generation (Go)

```go
import "github.com/google/uuid"

func generateUUIDv7() string {
    // uuid.NewV7() returns UUIDv7 using current timestamp
    id, err := uuid.NewV7()
    if err != nil {
        // Fallback to UUIDv4 if clock unavailable (should never happen)
        id = uuid.New()
    }
    return id.String()  // Canonical format: "018d3f72-1234-7abc-9def-0123456789ab"
}
```

### UUIDv7 Parsing and Validation

```go
func parseUUID(s string) (uuid.UUID, error) {
    id, err := uuid.Parse(s)  // Validates format, not timestamp
    if err != nil {
        return uuid.Nil, fmt.Errorf("invalid UUID: %w", err)
    }
    return id, nil
}
```

**No Timestamp Validation:** Server does not validate UUID timestamp against
server clock. Out-of-order UUIDs (due to clock skew) are accepted.

### UTF-8 Validation

**Go:** Strings are UTF-8 by default; no explicit validation needed.

**Database:** SQLite/PostgreSQL enforce UTF-8 encoding on TEXT columns.

**HTTP/gRPC:** UTF-8 encoding enforced by protocol (HTTP Content-Type, gRPC
Protobuf string type).

**Explicit Validation (when accepting external input):**

```go
import "unicode/utf8"

func validateUTF8(s string) error {
    if !utf8.ValidString(s) {
        return fmt.Errorf("invalid UTF-8 encoding")
    }
    return nil
}
```

Used for untrusted external input (file uploads, webhook payloads).

## Cross-References

- [Identifiers (UUIDv7)](../03-data/identifiers-uuidv7.md) - Complete UUIDv7
  strategy and implementation
- [Event Schema and Storage](../03-data/event-schema-storage.md) - UUIDv7 for
  event IDs, time-series storage
- [Timestamps](../03-data/timestamps.md) - Timestamp handling and UUIDv7 time
  extraction
- [Database Backend](../09-operations/database-backend.md) - UTF-8 encoding in
  SQLite/PostgreSQL

## Future Considerations

### Clock Drift Monitoring

Detect excessive clock skew:

- Monitor UUID timestamp vs server clock on event ingestion
- Alert if drift exceeds threshold (e.g., +/-1 second)
- Provides early warning of NTP misconfiguration

### UUID Timestamp Extraction

Expose timestamp from UUIDv7 for diagnostics:

```go
func extractTimestamp(id uuid.UUID) time.Time {
    // UUIDv7 first 48 bits are unix_ts_ms
    tsMs := int64(id[0])<<40 | int64(id[1])<<32 | int64(id[2])<<24 |
            int64(id[3])<<16 | int64(id[4])<<8 | int64(id[5])
    return time.UnixMilli(tsMs)
}
```

Useful for debugging clock skew, validating event ordering.

### High-Frequency Event Handling

For events arriving faster than 1ms (UUIDv7 precision):

- Use monotonic counter in `rand_a` field (12 bits, 4096 unique IDs per ms)
- Increment counter for IDs generated within same millisecond
- Ensures strict ordering even at microsecond event rates

Implementation deferred until event volume justifies complexity.
