---
doc_type: spoke
status: active
date_created: 2025-11-07
primary_category: architecture
hub_document: doc/03-data/README.md
tags:
  - timestamps
  - protobuf
  - time
  - utc
---

# Timestamp Representation

## Context

TrapperKeeper handles timestamps across multiple architectural boundaries (gRPC protocol, Go application code, database persistence) requiring consistent conversion utilities preserving precision while ensuring compatibility across SQLite, PostgreSQL, and MySQL backends.

Each layer uses optimal timestamp representation for that context: Protocol Buffers for wire format, time.Time for Go application logic, database TIMESTAMP types for persistence. Explicit conversion functions at boundaries prevent format mixing and maintain type safety.

**Hub Document**: This document is part of the Data Hub. See [Data Architecture](README.md) for strategic overview of multi-layer timestamp handling within TrapperKeeper's data model.

## Layer-Specific Representations

Different timestamp formats optimized for each architectural layer.

### Protocol Boundary (google.protobuf.Timestamp)

Wire format for gRPC communication:

```protobuf
import "google/protobuf/timestamp.proto";

message Event {
  string event_id = 1;
  google.protobuf.Timestamp client_timestamp = 2;
}
```

**Properties**:

- Nanosecond precision
- Language-agnostic serialization
- Protocol Buffers standard type
- Used in all `.proto` definitions

**Go Type**: `timestamppb.Timestamp`

```go
import "google.golang.org/protobuf/types/known/timestamppb"

// Timestamp represents seconds and nanoseconds
type Timestamp struct {
    Seconds int64  // Unix timestamp seconds
    Nanos   int32  // Nanosecond component (0-999,999,999)
}
```

**Cross-References**:

- API Service Architecture Section 2: Protocol buffer schemas
- Data Architecture Section 3: Multi-layer timestamp handling

### Application Layer (time.Time)

Go application code standard:

```go
import (
    "time"
    "encoding/json"
    "github.com/google/uuid"
)

type Event struct {
    EventID          uuid.UUID       `json:"event_id"`
    ClientTimestamp  time.Time       `json:"client_timestamp"` // Go application type
    Payload          json.RawMessage `json:"payload"`
}
```

**Properties**:

- encoding/json integration for JSON serialization
- database/sql integration for database mapping
- Type-safe operations (arithmetic, formatting)
- UTC timezone enforced via .UTC() method

**Benefits**:

- Ecosystem integration (standard Go type)
- Type safety prevents timezone mixing
- Efficient representation (int64 sec + int32 nsec)

**Cross-References**:

- Event Schema and Storage: Event timestamp fields

### Database Layer (TIMESTAMP types)

Database-specific storage with varying precision:

**PostgreSQL/MySQL**:

```sql
CREATE TABLE events (
    event_id UUID PRIMARY KEY,
    client_timestamp TIMESTAMP NOT NULL,  -- Microsecond precision
    server_received_at TIMESTAMP NOT NULL
);
```

**SQLite**:

```sql
CREATE TABLE events (
    event_id TEXT PRIMARY KEY,
    client_timestamp TEXT NOT NULL,  -- ISO8601, nanosecond precision
    server_received_at TEXT NOT NULL
);
```

**Precision**:

- PostgreSQL: Microsecond (6 decimal places)
- MySQL: Microsecond (6 decimal places)
- SQLite: Nanosecond (via TEXT ISO8601)

**database/sql Mapping**: Automatic conversion between `time.Time` and database TIMESTAMP

**Cross-References**:

- Database Backend: Database-specific type configuration

## Conversion Utilities

Explicit functions convert between representations at boundary crossings.

### Protobuf ↔ time.Time Conversion

`internal/grpc/conversions.go`:

```go
import (
    "time"
    "google.golang.org/protobuf/types/known/timestamppb"
)

// Convert protobuf Timestamp to time.Time
func timestampToTime(ts *timestamppb.Timestamp) time.Time {
    return ts.AsTime()
}

// Convert time.Time to protobuf Timestamp
func timeToTimestamp(t time.Time) *timestamppb.Timestamp {
    return timestamppb.New(t)
}
```

**Usage**:

```go
import (
    "context"
    "database/sql"
    pb "github.com/trapperkeeper/api/proto/gen/go"
)

// gRPC endpoint receives protobuf Event
func (s *Server) ReportEvents(
    ctx context.Context,
    req *pb.ReportEventsRequest,
) (*pb.ReportEventsResponse, error) {
    protoEvents := req.GetEvents()

    for _, protoEvent := range protoEvents {
        // Convert protobuf timestamp to time.Time
        clientTimestamp := protoEvent.GetClientTimestamp().AsTime()

        // Store in database (database/sql handles time.Time -> TIMESTAMP)
        _, err := s.db.Exec(
            "INSERT INTO events (event_id, client_timestamp) VALUES (?, ?)",
            protoEvent.GetEventId(),
            clientTimestamp,
        )
        if err != nil {
            return nil, err
        }
    }

    return &pb.ReportEventsResponse{}, nil
}
```

**Error Handling**: AsTime() returns zero time on invalid timestamps (check with IsZero() if needed).

**Cross-References**:

- API Service Architecture: ReportEvents RPC implementation
- Client/Server Package Separation: Conversion utility location

### Database Mapping (Automatic)

database/sql provides automatic conversion between `time.Time` and database TIMESTAMP:

```go
import (
    "time"
    "database/sql"
)

type Event struct {
    EventID          string    `db:"event_id"`
    ClientTimestamp  time.Time `db:"client_timestamp"` // Automatic mapping
}

// Query automatically converts TIMESTAMP -> time.Time
var event Event
err := db.QueryRow(
    "SELECT event_id, client_timestamp FROM events WHERE event_id = ?",
    eventID,
).Scan(&event.EventID, &event.ClientTimestamp)
if err != nil {
    return err
}
```

**No Explicit Conversion**: database/sql handles database ↔ time.Time conversion transparently.

**Cross-References**:

- Database Backend: database/sql configuration

## UTC Enforcement

All timestamps use UTC timezone eliminating timezone-related bugs.

### UTC-Only Policy

**Client SDKs**: Generate timestamps in UTC

```python
from datetime import datetime, timezone

# Correct: UTC timestamp
timestamp = datetime.now(timezone.utc)
sensor.observe({"timestamp": timestamp.isoformat()})

# Incorrect: Local timezone (rejected by server)
timestamp = datetime.now()  # NO
```

**Server**: Accepts only UTC timestamps

```go
func validateTimestamp(ts time.Time) error {
    // Convert to UTC and store
    utcTS := ts.UTC()
    // Store utcTS in database
    return nil
}
```

**Database**: Stores UTC timestamps

```sql
-- PostgreSQL: TIMESTAMP WITHOUT TIME ZONE (UTC assumed)
CREATE TABLE events (
    client_timestamp TIMESTAMP NOT NULL
);

-- SQLite: TEXT with explicit 'Z' suffix
CREATE TABLE events (
    client_timestamp TEXT NOT NULL  -- "2025-10-29T10:00:00.000Z"
);
```

**Benefits**:

- Eliminates daylight saving time bugs
- Consistent sorting across timezones
- Simplified arithmetic operations
- No ambiguous timestamps

**Cross-References**:

- Data Architecture Section 3: UTC timezone policy

## Precision Handling

Database precision varies requiring awareness of limits.

### Precision Loss

**Nanosecond -> Microsecond** (PostgreSQL/MySQL):

```go
original := time.Now().UTC() // Nanosecond precision
// -> 2025-10-29T10:00:00.123456789Z

// Store in PostgreSQL
_, err := db.Exec("INSERT INTO events (ts) VALUES (?)", original)
if err != nil {
    return err
}

// Retrieve from PostgreSQL
var stored time.Time
err = db.QueryRow("SELECT ts FROM events").Scan(&stored)
// -> 2025-10-29T10:00:00.123456Z (nanoseconds lost)
```

**Impact**: Nanosecond component truncated (999ns → 0ns)

**Mitigation**: UUIDv7 uses millisecond precision for ordering, so microsecond database precision is sufficient for TrapperKeeper use cases.

### SQLite Nanosecond Preservation

```go
// SQLite stores as TEXT ISO8601
original := time.Now().UTC() // 2025-10-29T10:00:00.123456789Z

_, err := db.Exec("INSERT INTO events (ts) VALUES (?)", original)
if err != nil {
    return err
}

var stored time.Time
err = db.QueryRow("SELECT ts FROM events").Scan(&stored)
// -> 2025-10-29T10:00:00.123456789Z (full nanosecond precision)
```

**Cross-References**:

- Database Backend: Database-specific timestamp handling
- Identifiers (UUIDv7): UUIDv7 millisecond precision

## Clock Drift Monitoring

Server validates client timestamps and logs warnings for drift.

### Drift Validation

```go
import (
    "fmt"
    "time"
    "log"
)

func validateTimestampDrift(clientTS time.Time) error {
    serverTS := time.Now().UTC()
    drift := serverTS.Sub(clientTS).Abs()
    driftMS := drift.Milliseconds()

    if driftMS > 100 {
        log.Printf(
            "Clock drift detected: client=%s, server=%s, drift=%dms",
            clientTS, serverTS, driftMS,
        )
    }

    if driftMS > 5000 {
        return fmt.Errorf("clock drift excessive: %dms", driftMS)
    }

    return nil
}
```

**Thresholds**:

- Warning: >100ms drift (logged for monitoring)
- Error: >5000ms drift (request rejected)

**Rationale**: Small drift acceptable for TrapperKeeper use cases. Large drift indicates misconfigured NTP or client clock issues.

**Cross-References**:

- Configuration Management: NTP synchronization requirements

## Testing Patterns

Round-trip tests verify precision preservation.

### Conversion Round-Trip

```go
import (
    "testing"
    "time"
)

func TestRoundTripConversion(t *testing.T) {
    original := time.Now().UTC()
    protoTS := timeToTimestamp(original)
    roundTrip := timestampToTime(protoTS)

    // Should preserve precision to nanosecond
    if !original.Equal(roundTrip) {
        t.Errorf("round trip failed: %v != %v", original, roundTrip)
    }
}
```

### Database Precision Test

```go
func TestDatabasePrecisionLoss(t *testing.T) {
    original := time.Now().UTC()
    stored := storeAndRetrieveTimestamp(db, original)

    // PostgreSQL/MySQL lose nanoseconds
    diffNanos := original.Sub(stored).Abs().Nanoseconds()
    if diffNanos >= 1000 { // Within 1 microsecond
        t.Errorf("precision loss too large: %d ns", diffNanos)
    }
}
```

**Cross-References**:

- Testing Integration Patterns: Database testing strategies

## Related Documents

**Dependencies** (read these first):

- Data Architecture: Multi-layer timestamp strategy
- Identifiers (UUIDv7): UUIDv7 timestamp component

**Related Spokes** (siblings in this hub):

- Event Schema and Storage: Timestamp field usage
- Client Metadata Namespace: System timestamp metadata

**Extended by**:

- Database Backend: Database timestamp type configuration
- API Service Architecture: Protobuf timestamp usage
