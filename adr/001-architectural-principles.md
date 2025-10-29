# ADR-001: Architectural Principles

Date: 2025-10-28

## Context

TrapperKeeper is a rule-based data observability system designed for high-throughput data pipelines. Before defining specific technical decisions, we must establish foundational principles that guide all architectural choices. These principles emerged from analyzing the target use cases:

- **Ephemeral workloads**: Data processing jobs (Airflow, Spark) run for minutes to hours, not days
- **Diverse data formats**: Industrial IoT generates 15+ schema types per customer, ranging from JSON to compressed Parquet with 500K-point waveforms
- **High throughput requirements**: Must process millions of records per day with <1ms overhead
- **Operational simplicity**: Five-engineer startup cannot maintain complex infrastructure
- **Fail-safe requirements**: Observability failures must never break production pipelines

Traditional observability systems assume persistent infrastructure, pre-registered schemas, and schema stability. TrapperKeeper operates in an environment where none of these assumptions hold.

## Decision

We will adopt five core architectural principles that override default behaviors and inform all other technical decisions.

### 1. Schema-Agnostic Architecture

**Principle**: The central server has zero understanding of data schemas.

**Implications**:
- No schema registry or pre-registration required
- Rules operate on abstract field paths resolved at runtime by SDKs
- System works with schemaless/agile data pipelines and structured data equally
- Framework adapters (Pandas, Spark) map their type systems to TrapperKeeper's field path model
- Server cannot validate field existence, only rule syntax

**Rationale**: Customers have 15+ diverse dataset types per deployment (compressed JSON, CSV, Parquet with 500K-point waveforms). Requiring schema registration creates deployment friction and becomes stale as schemas evolve. Schema-agnostic design aligns with modern agile data pipelines where schemas change frequently or don't exist at all.

**Referenced by**: ADR-002 (SDK Model), ADR-014 (Rule Expression Language), ADR-015 (Field Path Resolution)

### 2. Least Intrusive by Default

**Principle**: System degrades to pass-through rather than failing pipelines.

**Default behaviors**:
- **Network failures**: Sensors operate in "fail-safe" mode (disable all rules, become no-op)
- **Missing fields**: Rules skip rather than error (configurable via `on_missing_field`)
- **Type coercion failures**: Treat as condition failed, continue evaluation
- **Event POST failures**: Log warning, continue processing (observability shouldn't break pipelines)
- **Empty arrays with wildcards**: No match (consistent with ANY semantics)

**Rationale**: TrapperKeeper is an observability layer, not core business logic. Production pipelines must not fail because rule evaluation has issues. Engineers explicitly opt into strict mode (`on_missing_field="error"`, fail-closed sensors) when needed for critical validation.

**Referenced by**: ADR-002 (SDK Model), ADR-015 (Field Path Resolution), ADR-018 (Schema Evolution), ADR-021 (Failure Modes)

### 3. Ephemeral Sensors

**Principle**: Sensors are short-lived by design, tied to job lifecycles.

**Characteristics**:
- Sensors live for duration of data processing job (minutes to hours)
- No persistent identity or registration
- State limited to in-memory rule cache and event buffer
- Destroyed when job completes
- No concept of "sensor health monitoring" as persistent entities

**Operational perspective**:
- Operators view sensors through events: "what sensors reported in the past hour?"
- No heartbeats or persistent connections
- Sensors appear and disappear naturally with job execution

**Rationale**: Aligns with modern ephemeral compute patterns (containers, serverless, batch jobs). Simplifies state management—sensors naturally disappear when jobs complete. Eliminates need for registration lifecycle management, health checks, and stale sensor cleanup. Customers run Airflow/Spark jobs that start, process data, and terminate.

**Referenced by**: ADR-002 (SDK Model), ADR-005 (API Service Architecture), ADR-021 (Failure Modes)

### 4. Simplicity

**Principle**: Avoid over-engineering, defer complexity to future iterations.

**Scope constraints**:
- Single-tenant only (multi-tenancy in data model but not enforced)
- No staged rollouts, A/B testing, or rule versioning
- Simple validation (schema checks only, no logic validation)
- Accept "bloated" event storage if architecture allows later optimization
- Basic JSONL storage, migrate to time-series database later
- No automatic retention policies (manual deletion)
- Last-write-wins concurrency (no optimistic locking)
- In-memory rule caching only (no disk persistence)

**Rationale**: Five-engineer startup cannot maintain complex infrastructure. Build working system first, optimize later. Many "required" features (versioning, staged rollouts, schema validation) are actually optional for MVP. YAGNI principle applied aggressively—customers need basic rule evaluation, not enterprise feature set.

**Referenced by**: All ADRs that defer features or choose simpler implementations

### 5. Consistent Encoding and Identifiers

**Principle**: Use UTF-8 everywhere and UUIDv7 for all identifiers.

**UTF-8 encoding**:
- All strings stored as UTF-8
- User-generated content (Web UI): UTF-8
- Client/sensor data sent to server: UTF-8
- Language-specific conversions (Python UTF-16/32) handled transparently by libraries

**UUIDv7 identifiers**:
- All entities use UUIDv7: tenants, teams, users, rules, API keys, events
- **Benefits**: Sortable (time-ordered), globally unique, efficient for time-series data
- **Implementation**: Use native UUID type where supported (PostgreSQL, MySQL), otherwise string
- **Clock sync**: Requires NTP synchronization, accept client-generated UUIDs as-is
- **Validation**: Warn if client/server time differs by >100ms but don't reject

**Rationale**: Consistent encoding eliminates character set confusion. UUIDv7 provides natural time-ordering for events and rules, benefits time-series queries, and avoids distributed ID generation complexity. No need for centralized sequence generators or coordination.

**Referenced by**: ADR-003 (UUID Strategy), ADR-013 (Event Schema), ADR-014 (Rule Expression Language)

## Consequences

### Benefits

1. **Deployment Simplicity**: No schema pre-registration eliminates deployment step and keeps server stateless
2. **Pipeline Safety**: Fail-safe defaults prevent observability issues from breaking production data flows
3. **Operational Flexibility**: Ephemeral design aligns with container/serverless patterns, no cleanup required
4. **Development Velocity**: MVP simplicity enables fast iteration and reduces maintenance burden
5. **Data Consistency**: UTF-8 and UUIDv7 standards eliminate encoding bugs and ID conflicts
6. **Schema Evolution**: Schema-agnostic design handles changing schemas without server coordination

### Tradeoffs

1. **Limited Validation**: Cannot validate field existence on server side (SDK detects at runtime)
2. **Error Visibility**: Silent failures in fail-safe mode may hide configuration issues (mitigated by event logs)
3. **Feature Gaps**: MVP simplicity defers common enterprise features (versioning, rollouts, retention automation)
4. **Storage Inefficiency**: No normalization or optimization in initial implementation (acceptable tradeoff)
5. **Clock Dependency**: UUIDv7 requires NTP synchronization (not guaranteed in all environments)
6. **Migration Overhead**: Simple MVP implementations require later refactoring for scale

### Philosophical Context

These principles reflect **pragmatic minimalism**:
- Build working system before optimizing
- Fail gracefully rather than strictly
- Align with modern ephemeral infrastructure
- Eliminate unnecessary complexity
- Defer enterprise features until needed

This differs from traditional enterprise observability systems that assume persistent infrastructure, strict validation, and complex feature sets. TrapperKeeper optimizes for startup velocity and operational simplicity.

## Implementation

These principles manifest across all system components:

1. **SDK Design** (ADR-002):
   - Ephemeral sensors with fail-safe defaults
   - No schema validation, operates on runtime structures
   - Event buffering with explicit flush (no auto-magic)

2. **Rule Engine** (ADR-014, ADR-015, ADR-016):
   - Field paths resolved at runtime (schema-agnostic)
   - `on_missing_field="skip"` default (least intrusive)
   - Wildcard evaluation with ANY semantics (first-match short-circuit)

3. **Event Storage** (ADR-013):
   - JSONL files for MVP (simple, honest about scale)
   - UUIDv7 event IDs with client-generated timestamps
   - Full record/rule snapshots (bloated but complete audit trail)

4. **API Architecture** (ADR-005):
   - Stateless gRPC protocol (ephemeral-friendly)
   - No sensor registration or health checks
   - ETAG-based rule sync (simple cache invalidation)

5. **Failure Handling** (ADR-021):
   - Fail-safe by default when API unreachable
   - Network partitions result in disabled rules
   - No event buffering during outages

6. **Data Validation**:
   - UTF-8 everywhere with no control characters
   - Character set validation at API boundaries
   - No schema validation (client responsibility)

7. **Identifier Management** (ADR-003):
   - UUIDv7 for all entities (rules, events, tenants)
   - Clock drift warnings but not rejections
   - Native UUID types where supported

## Related Decisions

This ADR establishes foundational principles that guide all other architectural decisions:

- **ADR-002: SDK Model** - Implements the Ephemeral Sensors principle
- **ADR-003: UUID Strategy** - Implements the Consistent Encoding and Identifiers principle
- **ADR-004: Database Backend** - Implements the MVP Simplicity principle
- **ADR-005: API Service Architecture** - Implements Schema-Agnostic and Ephemeral Sensors principles
- **ADR-021: Failure Modes** - Implements the Least Intrusive by Default principle

All other ADRs inherit these principles either directly or through their parent decisions.

## Future Considerations

These principles may evolve as system matures:

1. **Strict Mode Environments** (optional enhancement):
   - Fail-closed by default for critical validation
   - Require schema registration
   - Type checking at rule definition time
   - Tradeoff: Reduces flexibility

2. **Multi-Tenant Enforcement**:
   - Tenant isolation and quotas
   - Per-tenant feature flags
   - Needed when cloud service is deployed

These principles prioritize **simplicity and safety** over **features and strictness**. Future enhancements should justify their complexity against operational burden.
