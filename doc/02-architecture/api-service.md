---
doc_type: spoke
status: active
date_created: 2025-11-07
primary_category: architecture
hub_document: doc/02-architecture/README.md
tags:
  - grpc
  - protobuf
  - stateless
  - etag
---

# API Service Architecture

## Context

The `tk-sensor-api` service provides gRPC communication between sensors and TrapperKeeper server. This service must handle high-throughput event ingestion from ephemeral sensors while maintaining stateless protocol design for horizontal scalability and operational simplicity.

The API uses Protocol Buffers for schema evolution support, ETAG-based synchronization to minimize bandwidth, and HMAC authentication for security. Protocol buffer compilation is handled by `buf` or `protoc` enabling both client (SDK) and server (API service) to share generated types.

**Hub Document**: This document is part of the Architecture Hub. See [Architecture Overview](README.md) for strategic context on the two-service model and how gRPC sensor communication fits within TrapperKeeper's architecture.

## gRPC Service Definition

The sensor API provides three core RPC methods defined in `proto/trapperkeeper/sensor/v1/sensor_api.proto`:

### SyncRules RPC

Synchronizes rules from server to sensor using ETAG-based conditional requests:

```protobuf
rpc SyncRules(SyncRulesRequest) returns (SyncRulesResponse) {}

message SyncRulesRequest {
  repeated string tags = 1;  // Rule scope tags
}

message SyncRulesResponse {
  repeated Rule rules = 1;   // Pre-compiled rules
  string etag = 2;           // Version fingerprint
}
```

**Protocol Flow**:

1. **Initial sync**: Sensor sends `SyncRulesRequest` with tags, receives all matching rules plus ETAG
2. **Subsequent syncs**: Sensor includes `if-none-match: <etag>` metadata header
3. **No changes**: Server returns empty `rules` array with same ETAG
4. **Changes detected**: Server returns updated rules with new ETAG

**ETAG Generation**: SHA256 hash of concatenated rule IDs + modified timestamps ensuring deterministic versioning.

**Benefits**: Minimizes bandwidth for frequent syncs, stateless protocol enables horizontal scaling, sensors control sync frequency.

**Cross-References**:

- SDK Model Section 3: Client-side ETAG caching implementation
- Rule Lifecycle: Rule state transitions affecting ETAG calculation
- Testing Examples Section 2: ETAG synchronization test

### ReportEvents RPC

Batch event submission from sensor to server:

```protobuf
rpc ReportEvents(ReportEventsRequest) returns (ReportEventsResponse) {}

message ReportEventsRequest {
  repeated Event events = 1;
  string sensor_id = 2;        // Sensor identifier
  google.protobuf.Timestamp client_timestamp = 3;
}

message ReportEventsResponse {
  repeated EventResult results = 1;
  int32 accepted_count = 2;
  int32 rejected_count = 3;
}

message EventResult {
  string event_id = 1;
  EventStatus status = 2;      // ACCEPTED, REJECTED, ERROR
  string error_message = 3;    // If status == ERROR
}
```

**Batch Processing**:

- SDK buffers events locally until explicit `flush()`
- Batch size configurable (default: 128 events)
- Partial failures supported: Some events succeed, others fail
- Results array provides per-event status

**Error Handling**:

- `ACCEPTED`: Event stored successfully
- `REJECTED`: Event failed validation (schema, metadata limits)
- `ERROR`: Server error during processing (retry recommended)

**Cross-References**:

- SDK Model Section 4: Explicit buffer management
- Event Schema and Storage: Event validation rules
- Batch Processing and Vectorization: Performance optimizations

### GetDiagnostics RPC

Retrieves SDK diagnostic information for troubleshooting:

```protobuf
rpc GetDiagnostics(GetDiagnosticsRequest) returns (GetDiagnosticsResponse) {}

message GetDiagnosticsRequest {
  string sensor_id = 1;
}

message GetDiagnosticsResponse {
  int32 buffered_events_count = 1;
  int32 rules_synced_count = 2;
  google.protobuf.Timestamp last_sync_time = 3;
  repeated string active_rule_ids = 4;
}
```

**Use Cases**:

- Debugging: Verify sensor has synced rules
- Monitoring: Check buffer size before flush
- Troubleshooting: Confirm rule synchronization completed

**Cross-References**:

- SDK Model Section 5: Diagnostic information exposure
- Health Check Endpoints: Health check integration

## Protocol Buffer Schemas

Complete protocol buffer definitions organized in `proto/trapperkeeper/sensor/v1/` directory.

### Rule Schema (rule.proto)

```protobuf
message Rule {
  string rule_id = 1;
  string name = 2;
  RuleState state = 3;
  Action action = 4;
  repeated OrGroup or_groups = 5;
  OnMissingField on_missing_field = 6;
  double sample_rate = 7;
  repeated ScopeTag scope_tags = 8;
  google.protobuf.Timestamp created_at = 9;
  google.protobuf.Timestamp modified_at = 10;
}

message OrGroup {
  int32 group_index = 1;
  repeated Condition conditions = 2;
}

message Condition {
  FieldPath field_path = 1;
  Operator op = 2;
  ConditionValue value = 3;
  FieldType field_type = 4;
}

enum RuleState {
  RULE_STATE_UNSPECIFIED = 0;
  RULE_STATE_DRAFT = 1;
  RULE_STATE_ACTIVE = 2;
  RULE_STATE_DISABLED = 3;
}

enum Action {
  ACTION_UNSPECIFIED = 0;
  ACTION_OBSERVE = 1;
  ACTION_DROP = 2;
}

enum OnMissingField {
  ON_MISSING_FIELD_UNSPECIFIED = 0;
  ON_MISSING_FIELD_SKIP = 1;
  ON_MISSING_FIELD_ERROR = 2;
  ON_MISSING_FIELD_NULL = 3;
}
```

**Design Rationale**:

- DNF representation: `or_groups` contain `conditions` (AND within group, OR between groups)
- Explicit ordering: `group_index` ensures deterministic evaluation
- State machine: `DRAFT → ACTIVE → DISABLED` lifecycle
- Missing field handling: Configurable per rule via `on_missing_field`

**Cross-References**:

- Rule Expression Language: DNF evaluation semantics
- Rule Lifecycle: State transition rules
- Schema Evolution: Missing field handling strategies

### Field Path Schema (field_path.proto)

```protobuf
message FieldPath {
  repeated PathSegment segments = 1;
}

message PathSegment {
  oneof segment_type {
    string field_name = 1;        // Object key: $.user.name
    int32 array_index = 2;        // Array index: $.users[0]
    WildcardSegment wildcard = 3; // Wildcard: $.users[*]
  }
}

message WildcardSegment {
  // Empty message: presence indicates wildcard
}
```

**Path Resolution**:

- `$.user.name`: Two segments (field_name: "user", field_name: "name")
- `$.users[0]`: Two segments (field_name: "users", array_index: 0)
- `$.users[*].age`: Three segments (field_name: "users", wildcard, field_name: "age")

**Wildcard Semantics**: Expands to all array elements or object values, result is array of matched values.

**Cross-References**:

- Field Path Resolution: Runtime resolution algorithm
- Performance Model and Optimization: Nested wildcard validation limits

### Event Schema (event.proto)

```protobuf
message Event {
  string event_id = 1;                           // UUIDv7
  google.protobuf.Struct payload = 2;            // Arbitrary JSON
  map<string, string> metadata = 3;              // User metadata
  google.protobuf.Timestamp client_timestamp = 4;
}
```

**Payload Field**: Uses `google.protobuf.Struct` for arbitrary JSON preserving schema-agnostic principle.

**Metadata Field**: String key-value pairs with limits (64 pairs, 128-char keys, 1KB values, 64KB total).

**Client Timestamp**: Preserves sensor-side timestamp for event correlation and audit trail.

**Cross-References**:

- Event Schema and Storage: Complete event validation rules
- Client Metadata Namespace: `$tk.*` prefix reservation and limits
- Timestamp Representation: Conversion between protobuf and Go types

## Protocol Buffer Compilation

Protocol buffer compilation uses `buf` or `protoc` with `grpc-go` plugin.

### Build Configuration

Using `buf` (recommended):

```yaml
# buf.gen.yaml
version: v2
plugins:
  - remote: buf.build/protocolbuffers/go
    out: gen/go
    opt:
      - paths=source_relative
  - remote: buf.build/grpc/go
    out: gen/go
    opt:
      - paths=source_relative
```

Using `protoc` directly:

```bash
protoc \
  --go_out=gen/go --go_opt=paths=source_relative \
  --go-grpc_out=gen/go --go-grpc_opt=paths=source_relative \
  proto/trapperkeeper/sensor/v1/*.proto
```

**Key Features**:

- Both client and server stubs generated from single compilation
- Proto files live in `proto/` directory at repository root
- Generated code placed in `gen/go/` directory
- All consumers (SDK, API service) import generated types from shared package

**Cross-References**:

- Monorepo Directory Structure: Go module organization
- Protobuf code generation workflow
- Timestamp Representation: Protobuf timestamp conversion utilities

## Stateless Protocol Design

API service maintains no session state enabling horizontal scalability.

### Authentication per Request

Every RPC includes HMAC signature in metadata header:

```
Authorization: HMAC-SHA256 api_key_id:timestamp:signature
```

Server validates signature using stored HMAC secret, no session storage required.

**Benefits**:

- Horizontally scalable: Any instance can handle any request
- No sticky sessions or session replication needed
- Simplified load balancing
- Crash recovery trivial (no state to restore)

**Cross-References**:

- API Authentication: Complete HMAC validation algorithm
- Configuration Management: HMAC secret loading from environment

### ETAG-Based Caching

Rule synchronization uses ETAG headers for conditional requests:

1. Server computes ETAG from rule state (SHA256 of rule IDs + timestamps)
2. Client caches ETAG and includes in subsequent requests
3. Server compares: If match, returns empty response (304 equivalent)
4. If mismatch, returns updated rules with new ETAG

**Stateless Property**: ETAG deterministically computed from database state, no caching layer required.

**Cross-References**:

- SDK Model Section 3: Client-side ETAG caching
- Testing Integration Patterns Section 3: ETAG synchronization testing

## Error Handling

gRPC status codes map to application-level errors:

- `OK`: Successful request
- `UNAUTHENTICATED`: Invalid HMAC signature or expired API key
- `PERMISSION_DENIED`: Valid auth but insufficient permissions (future multi-tenancy)
- `INVALID_ARGUMENT`: Malformed request (invalid protobuf, missing required fields)
- `RESOURCE_EXHAUSTED`: Rate limit exceeded (future implementation)
- `UNAVAILABLE`: Database connection failure, retry recommended
- `INTERNAL`: Unexpected server error, do not retry

**Error Details**: Use `google.rpc.Status` for structured error information including field-level validation errors.

**Cross-References**:

- Error Handling Strategy: Unified error taxonomy and response patterns
- Failure Modes and Degradation: Client-side failure handling

## Performance Characteristics

API service targets:

- **Throughput**: 10,000 events/second per instance
- **Latency**: p99 < 100ms for `ReportEvents` RPC
- **Batch size**: 128 events default (configurable)
- **Rule sync**: p99 < 50ms for ETAG cache hit

**Optimization Strategies**:

- Connection pooling: 16 database connections per instance
- Concurrent processing: goroutines for non-blocking operations
- Batch inserts: Use prepared statements with batch parameters
- ETAG caching: Eliminates rule serialization overhead

**Cross-References**:

- Performance Model and Optimization: Complete performance model
- Sampling and Performance Optimization: Probabilistic sampling strategies

## Related Documents

**Dependencies** (read these first):

- Architecture Overview: Two-service model and protocol boundaries
- Principles Architecture: Schema-agnostic and ephemeral sensor principles

**Related Spokes** (siblings in this hub):

- SDK Model: Client-side implementation using this API
- Binary Distribution Strategy: Subcommand packaging

**Extended by**:

- API Authentication: HMAC authentication implementation
- TLS/HTTPS Strategy: gRPC TLS configuration
- Operational Endpoints: Health check endpoints for this service
