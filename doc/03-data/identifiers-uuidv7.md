---
doc_type: spoke
status: active
primary_category: database
hub_document: doc/03-data/README.md
tags:
  - uuidv7
  - identifiers
  - time-ordered
---

# Identifiers (UUIDv7)

## Context

TrapperKeeper requires globally unique identifiers that are time-ordered for efficient database indexing and chronological querying. Traditional UUID formats (v4 random, v1 timestamp+MAC) have significant drawbacks: v4 causes index fragmentation, v1 leaks hardware information.

UUIDv7 provides time-ordered identifiers with strong randomness, enabling efficient B-tree indexing without coordination overhead. All system identifiers (events, rules, users, sensors, tenants) use UUIDv7 format.

**Hub Document**: This document is part of the Data Hub. See [Data Architecture](README.md) for strategic overview of UUIDv7 within TrapperKeeper's identifier strategy.

## UUIDv7 Format

UUIDv7 combines millisecond timestamp with random bits for time-ordered uniqueness.

### Bit Layout

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                    unix_ts_ms (48 bits)                       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
| ver |          rand_a (12 bits)   |var|   rand_b (62 bits)    |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

**Fields**:

- `unix_ts_ms` (48 bits): Unix timestamp in milliseconds
- `ver` (4 bits): Version field (`0111` = 7)
- `rand_a` (12 bits): Random data
- `var` (2 bits): Variant field (`10`)
- `rand_b` (62 bits): Random data

**Total**: 48-bit timestamp + 74-bit random = 122 bits of useful data

**String Representation**: `018c9f8e-1234-7abc-9def-0123456789ab`

**Cross-References**:

- Data Architecture Section 2: UUIDv7 strategy overview
- Event Schema and Storage: UUIDv7 usage in event IDs

## Generation Algorithm

Go implementation using `github.com/google/uuid`:

```go
import (
    "time"
    "github.com/google/uuid"
)

// Generate UUIDv7 (most common)
id := uuid.Must(uuid.NewV7())
// -> 018c9f8e-1234-7abc-9def-0123456789ab

// Extract timestamp
timestamp := id.Time()
// -> time.Time{...}

// Format as RFC3339
dt := timestamp.UTC().Format(time.RFC3339Nano)
// -> 2025-10-29T10:00:00.123Z
```

**Generation Steps**:

1. Get current system time in milliseconds
2. Generate 74 random bits using cryptographically secure RNG
3. Combine timestamp (48 bits) + version (4 bits) + random (74 bits)
4. Format as UUID string

**Collision Resistance**:

- Same millisecond: 74 random bits provide 2^74 unique IDs
- Probability: 1 in 10^22 for collision in same millisecond
- Practical: Zero collisions for typical workloads

**Cross-References**:

- Timestamp Representation: Timestamp extraction from UUIDv7

## Identifier Types

All system entities use UUIDv7 identifiers.

### Event Identifiers

```go
import (
    "time"
    "encoding/json"
    "github.com/google/uuid"
)

type Event struct {
    EventID          uuid.UUID       `json:"event_id"`           // UUIDv7
    ClientTimestamp  time.Time       `json:"client_timestamp"`
    Payload          json.RawMessage `json:"payload"`
}

// Generate event ID
eventID := uuid.Must(uuid.NewV7())
```

**Properties**:

- Generated server-side on event ingestion
- Time-ordered by ingestion time
- Unique across all events globally
- Primary key in database

### Rule Identifiers

```go
type Rule struct {
    RuleID    uuid.UUID `json:"rule_id"`    // UUIDv7
    Name      string    `json:"name"`
    Expression string   `json:"expression"`
    CreatedAt time.Time `json:"created_at"`
}

// Generate rule ID
ruleID := uuid.Must(uuid.NewV7())
```

**Properties**:

- Generated server-side on rule creation
- Time-ordered by creation time
- Enables rule versioning (future)

### User and Tenant Identifiers

```go
type User struct {
    UserID   uuid.UUID `json:"user_id"`   // UUIDv7
    TenantID uuid.UUID `json:"tenant_id"` // UUIDv7
    Username string    `json:"username"`
}
```

**Multi-Tenancy Preparation**:

- All entities include `tenant_id` (UUIDv7)
- Single-tenant MVP uses default tenant
- Future multi-tenancy requires no schema changes

**Cross-References**:

- Database Backend: Multi-tenancy schema design

## Database Performance

UUIDv7 provides superior database indexing compared to UUIDv4.

### B-Tree Index Benefits

**UUIDv7** (time-ordered):

```
Index Node:
[018c9f8e-0000...] [018c9f8e-1000...] [018c9f8e-2000...]
```

- Sequential inserts append to rightmost leaf
- No page splits or reorganization
- High cache hit ratio

**UUIDv4** (random):

```
Index Node:
[7d3e2f1a-...] [a1b2c3d4-...] [2f3e4d5c-...]
```

- Random inserts cause page splits
- Frequent index reorganization
- Poor cache hit ratio

**Performance Impact**:

- UUIDv7 inserts: ~2x faster than UUIDv4
- Index fragmentation: ~90% reduction
- Query performance: No significant difference

**Cross-References**:

- Database Backend: Index optimization strategies
- Event Schema and Storage: Database indexing approach

## Time Ordering Guarantees

UUIDv7 provides chronological sorting with caveats.

### Monotonicity

**Within single process**:

```go
id1 := uuid.Must(uuid.NewV7())
time.Sleep(1 * time.Millisecond)
id2 := uuid.Must(uuid.NewV7())

// Guaranteed (assuming clock doesn't go backwards)
if id1.String() >= id2.String() {
    panic("monotonicity violated")
}
```

**Across distributed systems**:

```go
// Server A
idA := uuid.Must(uuid.NewV7())

// Server B (different machine)
idB := uuid.Must(uuid.NewV7())

// Ordering depends on clock synchronization
// Not guaranteed if clocks differ by >1ms
```

**Clock Drift Handling**:

- NTP synchronization required for accurate ordering
- Tolerance: ±100ms acceptable for TrapperKeeper use cases
- Warning logged if client/server time differs by >100ms

**Cross-References**:

- Timestamp Representation: Clock drift monitoring
- Configuration Management: NTP requirements

## Edge Cases and Limitations

Known limitations and mitigation strategies.

### Clock Rollback

**Scenario**: System clock moves backwards (NTP correction, manual adjustment)

**Behavior**: UUIDs generated during rollback appear older than they are

**Mitigation**:

- `github.com/google/uuid` detects rollback and maintains monotonicity
- Uses counter to ensure uniqueness within same millisecond
- Logs warning for monitoring

### High-Frequency Generation

**Scenario**: Generating >2^74 UUIDs in same millisecond (impossible in practice)

**Theoretical Collision**: 1 in 10^22 probability

**Practical**: TrapperKeeper generates ~10,000 events/second = 10 per millisecond, well below collision threshold

### Cross-System Ordering

**Scenario**: Events from sensors with unsynchronized clocks

**Limitation**: UUIDv7 ordering reflects client clock, not true chronological order

**Mitigation**:

- Store `server_received_at` timestamp for server-side ordering
- Use `client_timestamp` for client-side correlation
- Log warnings for clock drift >100ms

**Cross-References**:

- Event Schema and Storage: Multiple timestamp fields

## Related Documents

**Dependencies** (read these first):

- Data Architecture: UUIDv7 strategic overview

**Related Spokes** (siblings in this hub):

- Event Schema and Storage: UUIDv7 usage in event IDs
- Timestamp Representation: Timestamp extraction from UUIDv7

**Extended by**:

- Database Backend: Database indexing with UUIDv7
