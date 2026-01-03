---
doc_type: spoke
hub_document: doc/01-principles/README.md
status: active
date_created: 2026-01-02
primary_category: architecture
title: Simplicity (Pragmatic Minimalism)
tags:
  - mvp
  - yagni
  - complexity
  - single-tenant
---

# Simplicity (Pragmatic Minimalism)

## Core Principle

**Avoid over-engineering, defer complexity to future iterations.**

Five-engineer startup cannot maintain complex infrastructure. Build working
system first, optimize later. YAGNI (You Aren't Gonna Need It) principle applied
aggressively.

## Motivation

Startups die from premature optimization, not feature gaps. Complex systems
require ongoing maintenance:

- Multi-tenant isolation adds 3x development time
- Advanced concurrency (optimistic locking, versioning) adds debugging overhead
- Sophisticated database features (triggers, stored procedures) create lock-in
- Premature scaling architecture (microservices, message queues) adds
  operational burden

**Strategy:** Ship working MVP, iterate based on real customer pain points, not
hypothetical scale requirements.

## Scope Constraints

### Single-Tenant Only

Data model supports multi-tenancy (tenant_id columns exist), but enforcement
deferred:

**Current State:**

- Tenant isolation not enforced in queries (queries filter by tenant_id, but no
  row-level security)
- No tenant quotas or rate limiting
- No per-tenant feature flags
- Single-tenant deployment assumed

**Future Work:**

- Row-level security policies when cloud service deployed
- Tenant-level resource quotas (API rate limits, event storage caps)
- Feature flags for A/B testing across tenants

**Rationale:** On-premise single-tenant deployment doesn't need isolation.
Cloud-hosted SaaS will require enforcement, but that's 6+ months away.

### No Staged Rollouts

Rules applied immediately to all sensors:

**Current State:**

- Rule creation/update takes effect immediately
- No gradual rollout (e.g., 10% of sensors, then 50%, then 100%)
- No A/B testing framework (compare rule variants)
- No rule versioning (cannot rollback to previous rule definition)

**Future Work:**

- Canary deployment for rules (test on subset of sensors before full rollout)
- Rule versioning with rollback capability
- A/B testing framework for comparing rule variants

**Rationale:** Five engineers cannot afford sophisticated deployment machinery.
Immediate application is simpler and sufficient for MVP.

### Simple Database Validation

Application-layer validation in `tk-types`, minimal database-layer enforcement:

**Current State:**

- Foreign key constraints enforced (referential integrity)
- Unique indexes enforced (prevent duplicates)
- No CHECK constraints (e.g., no `CHECK (timeout_ms > 0)`)
- No database triggers
- No stored procedures

**Future Work:**

- CHECK constraints for invariants (non-negative values, enum validation)
- Triggers for audit logging or denormalization
- Stored procedures for complex multi-table operations

**Rationale:** Database-level logic is hard to test, debug, and migrate.
Application-layer validation in `tk-types` is sufficient and more flexible (see
[Unified Validation Strategy](../07-validation/README.md)).

**Important Distinction:** "Simple database-layer validation" means no
database-level logic (CHECK constraints, triggers). Application-layer validation
in `tk-types` can be sophisticated -- the complexity is in testable code, not
database schema.

### Basic JSONL Storage

Event storage uses append-only JSONL files:

**Current State:**

- Events written as newline-delimited JSON to files
- No compression (raw JSON text)
- No indexing (linear scan for queries)
- No partitioning (single file per day)
- Manual file rotation (separate operational process)

**Future Work:**

- Migrate to time-series database (InfluxDB, TimescaleDB, ClickHouse)
- Compression (gzip, lz4)
- Columnar storage for efficient queries
- Automatic partitioning and retention policies

**Rationale:** JSONL is simplest possible storage format. Sufficient for MVP
with low event volume (< 1M events/day). Migration path clear when performance
becomes bottleneck.

### No Automatic Retention Policies

Event data accumulates until manual deletion:

**Current State:**

- Events stored indefinitely (no TTL)
- Manual deletion via CLI or direct file removal
- No automatic cleanup based on age or volume

**Future Work:**

- Configurable retention policies (e.g., keep 30 days)
- Automatic cleanup background job
- Soft delete with grace period before purge

**Rationale:** Retention policies are operational complexity (background jobs,
monitoring, error handling). Manual cleanup sufficient for MVP.

**Important Distinction:** "No automatic retention policies" applies to event
data only. Infrastructure concerns like session cleanup (hourly auth token
expiration) are separate operational requirements and are implemented where
needed for security.

### Last-Write-Wins Concurrency

Concurrent updates to same entity use simple last-write-wins:

**Current State:**

- No optimistic locking (no version field)
- No pessimistic locking (no row-level locks held across transactions)
- Concurrent updates: last writer wins, earlier writes lost
- No conflict detection or resolution

**Future Work:**

- Optimistic locking with version field (detect concurrent updates, return
  conflict error)
- Conflict resolution UI (show conflicting changes, let user merge)
- Pessimistic locking for critical operations (API key rotation)

**Rationale:** Rule updates are infrequent (minutes to hours between changes).
Concurrent updates rare in practice. Optimistic locking adds complexity
(version field, conflict handling) with minimal benefit for MVP.

## Benefits

1. **Development Velocity**: Ship features in days, not weeks
2. **Reduced Maintenance**: Less code, fewer bugs, simpler operations
3. **Clear Migration Path**: Simple MVP establishes patterns, complex features
   added incrementally
4. **Easier Debugging**: Minimal moving parts, straightforward failure modes
5. **Fast Iteration**: Low cost to change direction based on customer feedback

## Tradeoffs

1. **Feature Gaps**: MVP lacks enterprise features (multi-tenancy, versioning,
   advanced concurrency)
2. **Storage Inefficiency**: JSONL format wastes disk space and query
   performance compared to columnar storage
3. **Migration Overhead**: Simple MVP implementations require later refactoring
   (e.g., JSONL -> time-series DB)
4. **Operational Burden**: Manual retention policies, no automatic cleanup
5. **Concurrency Risks**: Last-write-wins can lose updates in rare concurrent
   scenarios
6. **Limited Observability**: No canary deployments, no A/B testing, no rollback
   capability

## Implementation Guidelines

### When to Add Complexity

Complexity justified when:

1. **Customer Pain Point**: Multiple customers requesting feature
2. **Operational Burden**: Manual workaround taking significant engineering time
3. **Performance Bottleneck**: Simple approach measurably inadequate (profiling
   data required)
4. **Security Risk**: Simplicity creates exploitable vulnerability

Complexity NOT justified when:

1. **Hypothetical Scale**: "We might need this if we 10x"
2. **Best Practice**: "Industry standard is to use X"
3. **Resume-Driven Development**: "I want to learn technology Y"
4. **Premature Optimization**: "This could be faster if we..."

### Simplicity Checklist

Before adding a feature, verify:

- [ ] Is there a simpler approach that solves 80% of the use case?
- [ ] Can we defer this until customers ask for it?
- [ ] What is the ongoing maintenance cost (testing, documentation, operations)?
- [ ] Does this create new failure modes or operational complexity?
- [ ] Can we use an existing library/service instead of building it?

## Cross-References

- [Database Backend](../09-operations/database-backend.md) - SQLite default for
  zero-configuration
- [Event Schema and Storage](../03-data/event-schema-storage.md) - JSONL files
  for MVP
- [Unified Validation Strategy](../07-validation/README.md) -
  Application-layer validation complexity vs. simple database constraints
- [Database Migrations](../09-operations/database-migrations.md) - Simple
  migration strategy for evolving schema

## Future Considerations

### When to Migrate from Simplicity

Monitor these signals for when MVP simplicity becomes limiting:

**Multi-Tenancy:**

- On-premise deployments requesting tenant isolation
- Cloud SaaS launch imminent
- Security audit requires row-level security

**Rule Versioning:**

- Customers reporting production incidents from bad rule updates
- Requests for "test rule before deploying"
- Need for rollback capability

**Database Constraints:**

- Application-layer validation bugs causing invalid data in database
- Performance overhead from redundant validation
- Need for database-enforced invariants for data integrity

**JSONL Storage:**

- Event volume exceeds 1M/day (linear scan performance unacceptable)
- Disk space concerns (compression needed)
- Query latency SLA violations (indexing required)

**Retention Policies:**

- Disk space running out from unbounded event growth
- Manual cleanup consuming engineering time
- Compliance requirements for data deletion

**Concurrency:**

- Concurrent rule update conflicts reported by users
- Data loss from last-write-wins causing customer escalations
- Need for conflict detection and resolution UI

### Migration Strategy

When complexity justified, follow incremental migration:

1. **Design Backward-Compatible Change**: New feature coexists with old
   behavior
2. **Feature Flag**: Control rollout, easy rollback
3. **Incremental Rollout**: Test with internal usage, then alpha customers, then
   GA
4. **Document Migration Path**: Clear upgrade instructions for existing
   deployments
5. **Maintain Simple Default**: Advanced features opt-in, not mandatory

**Example: Optimistic Locking Migration**

```sql
-- Phase 1: Add version column (nullable, default NULL)
ALTER TABLE rules ADD COLUMN version INTEGER;

-- Phase 2: Application writes version on updates (if version present)
-- Last-write-wins behavior preserved when version=NULL

-- Phase 3: Backfill existing rows with version=1
UPDATE rules SET version = 1 WHERE version IS NULL;

-- Phase 4: Make version NOT NULL (new default: all rows have version)
ALTER TABLE rules ALTER COLUMN version SET NOT NULL;

-- Phase 5: Enable conflict detection (return 409 Conflict on version mismatch)
```

Each phase is independently deployable, rollback-safe, and maintains
compatibility.
