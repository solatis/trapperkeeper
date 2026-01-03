---
doc_type: spoke
status: active
primary_category: architecture
hub_document: doc/04-rule-engine/README.md
tags:
  - lifecycle
  - immutable
  - operational-controls
  - rule-management
cross_cutting:
  - validation
---

# Rule Lifecycle and Operational Controls

## Context

TrapperKeeper rules run against live production data pipelines, making operational safety critical. Rules can inadvertently drop valid data, cause pipeline failures through error actions, or degrade performance. Production incidents require rapid response to disable problematic rules without permanent deletion, while new rules need validation against live traffic without impact.

**Hub Document**: This document is part of the Rule Engine Architecture. See [Rule Engine Architecture](README.md) for strategic overview and relationships to expression language, field resolution, type coercion, and schema evolution.

## Immutable Rules Design

Rules are immutable: every modification creates a new rule record with a new `rule_id`. Old versions are preserved until retention cleanup. This design follows functional programming principles where state changes create new values rather than mutating existing ones.

### Design Rationale

**Why immutable rules?**

- **Stateless reasoning**: Immutable data is simpler to reason about -- no hidden state changes, no race conditions on updates
- **ETAG simplicity**: `ETAG = SHA256(sorted active rule_ids)` -- any rule change creates new ID, ETAG automatically changes
- **Audit trail built-in**: All rule versions preserved until retention cleanup, events reference exact version evaluated
- **Simplified concurrency**: Append-only avoids complex locking -- concurrent edits may create orphan versions (see Explicit Non-Goals)
- **No database triggers**: Simple application logic, no reliance on complex database internals

**What we avoid:**

- No `modified_at` timestamp tracking (rules never modified)
- No database triggers for timestamp updates
- No query-time transformations for dry-run mode
- No optimistic locking or version counters

### Rule Schema

```sql
CREATE TABLE rules (
  rule_id UUID PRIMARY KEY,      -- Immutable, unique per version (UUIDv7)
  name TEXT NOT NULL,
  action TEXT NOT NULL,          -- 'observe', 'drop', 'error'
  conditions TEXT NOT NULL,      -- JSON-encoded DNF conditions
  enabled BOOLEAN NOT NULL DEFAULT TRUE,
  deleted_at TIMESTAMP,          -- Soft-delete timestamp
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  created_by TEXT,               -- Operator identity
  INDEX idx_enabled_deleted (enabled, deleted_at)
);
```

- `rule_id`: Unique per version, immutable, used in events and ETAG
- No version tracking, lineage grouping, or rollback mechanism (see Explicit Non-Goals)

### Version Creation Flow

When operator modifies a rule:

1. Application soft-deletes old rule record (`deleted_at = NOW()`)
2. Application creates new rule record with new `rule_id`
3. ETAG changes automatically (new rule_id in hash)
4. Sensors receive new rules on next sync (0-30s)

```
rule_id=AAA, action=observe  (active)
    |
    v (operator changes action to 'drop')
rule_id=AAA soft-deleted, rule_id=BBB created with action=drop  (active)
    |
    v (operator adjusts conditions)
rule_id=BBB soft-deleted, rule_id=CCC created  (active)
```

**Active rules query**:

```sql
SELECT * FROM rules
WHERE enabled = TRUE
  AND deleted_at IS NULL;
```

### ETAG Computation

ETAG is deterministic hash of active rule IDs:

```go
func computeETAG(rules []Rule) string {
    ids := make([]string, len(rules))
    for i, r := range rules {
        ids[i] = r.RuleID.String()
    }
    sort.Strings(ids)
    hash := sha256.Sum256([]byte(strings.Join(ids, ",")))
    return hex.EncodeToString(hash[:])
}
```

**Properties**:

- Any new rule version = new rule_id = new ETAG
- Any rule deletion = rule_id removed = new ETAG
- Deterministic: same set of active rules = same ETAG
- No timestamp component needed

## Production Testing (Replacing Dry-Run)

The immutable design eliminates the need for a `dry_run` boolean. Production testing uses the standard version workflow with `action: observe`.

### Testing Workflow

1. **Create observe version**: New rule with `action: observe`
2. **Monitor events**: Events show rule matched with observe action
3. **Analyze impact**: "This rule would have dropped 1000 records"
4. **Promote to production**: Create new version with `action: drop`
5. **Deactivate test version**: Set `deleted_at` on observe version

```
Testing:    rule_id=AAA, action=observe  (active)
               |
               v (satisfied with testing)
AAA soft-deleted, rule_id=BBB created with action=drop  (active)
```

**Benefits over dry_run boolean**:

- System is action-agnostic (no special dry_run transformation logic)
- Events capture exact rule state (action=observe recorded directly)
- No ETAG invalidation ambiguity (new version = new ETAG)
- Simpler API (no dry_run field to track)

**Web UI workflow**:

- "Test this rule" button creates observe-action version
- Event viewer shows "observe" action directly (no transformation)
- "Promote to production" creates new version with real action
- "Discard test" sets deleted_at on observe version

## Emergency Pause All Rules

Global kill switch for production incidents:

**Implementation**:

- Server maintains in-memory boolean flag: `rulesGloballyPaused`
- Flag toggled via POST `/api/admin/rules/pause` and `/api/admin/rules/resume`
- When paused, API server returns empty rule set with special ETAG: `"PAUSED"`
- Sensors receive empty array, effectively disabling all rules

**API response when paused**:

```json
{
  "rules": [],
  "etag": "PAUSED",
  "paused": true
}
```

**Sensor behavior**:

- On empty rule set: Clear cached rules, evaluate no predicates
- SDK operates as pass-through (no observe/drop/error actions)
- Next sync interval (30s default): Check if rules resumed

**Rationale**:

- No database writes required (instant response, no rollback needed)
- Flag stored in memory only (resets on service restart to FALSE, fail-safe default)
- ETAG mechanism ensures rapid propagation to all sensors
- Explicit "paused" field in response prevents confusion with "no rules configured" state

**Observability**:

- Pause/resume actions logged with operator identity and timestamp
- Web UI displays prominent banner when globally paused
- Prometheus metric: `trapperkeeper_rules_globally_paused` (0 or 1)

## Individual Rule Enable/Disable

Toggle rules without deletion, preserving configuration:

**Implementation**:

- Rules have `enabled BOOLEAN NOT NULL DEFAULT TRUE` column
- Disabled rules filtered out in API query: `WHERE enabled = TRUE AND deleted_at IS NULL`
- Never sent to sensors (not included in ETAG calculation)
- Preserved in database for re-enabling later

**State transitions**:

```
CREATE  -> enabled=true, deleted_at=NULL  (default)
DISABLE -> enabled=false                   (hidden from sensors)
ENABLE  -> enabled=true                    (visible to sensors)
DELETE  -> deleted_at=NOW()                (soft-delete, cleanup later)
```

**Rationale**:

- Safer than deletion (configuration preserved)
- Enable/disable is in-place boolean update (exception to immutability for operational controls)
- Clear distinction between "temporarily disabled" and "deleted"
- Simple boolean filter in query (no performance impact)

## Data Retention

Database retention is independent of event storage retention. Both use 28-day default windows with daily cleanup.

### Retention Model

**Event storage**: 28-day retention, events older than window deleted first.

**Database rules**: Rules with `deleted_at` older than retention window AND no remaining event references are permanently deleted.

**Cleanup order** (critical for referential integrity):

1. Delete events older than 28 days
2. Delete rules where `deleted_at < NOW() - 29 days` AND `rule_id NOT IN (SELECT DISTINCT rule_id FROM events)`

The 29-day buffer (vs 28-day event retention) prevents race conditions where an event is created just as cleanup runs.

### Storage Estimates

At 10K logical rules, 10 changes/rule/month, 28-day retention:

- Rule versions: ~10K \* 10 = 100K records
- Storage: 100K \* 5KB average = 500MB
- Negligible compared to event storage

### Independence Principle

**Critical design requirement**: Event storage MUST NOT depend on database rule data.

Events embed complete rule snapshots at evaluation time. This separation enables:

- Independent retention policies (events may be kept longer than rule versions)
- Event storage can be queried without database access
- Audit trail self-contained in event records
- Database can be rebuilt from events if needed

See Event Schema and Storage for rule snapshot requirements.

## Change Propagation Timeline

Rule changes propagate via eventual consistency model:

**Timeline for rule update**:

1. Operator saves rule in Web UI (t=0s)
2. New rule version created, database transaction commits (t=0.05s)
3. Sensor polls API at next sync interval (t=0-30s, avg 15s)
4. Sensor receives new ETAG, fetches updated rules (t=15.1s)
5. Sensor applies updated rules to next record (t=15.1s)

**Implications**:

- No guarantee of instant propagation
- Sensors may evaluate records using previous rule version for up to 30 seconds
- Events capture exact rule snapshot, preserving audit trail across propagation delay
- Emergency pause propagates at next sync (up to 30s delay)

**Design Philosophy - Eventual Consistency**:

TrapperKeeper is explicitly designed as a loosely coupled distributed system that embraces eventual consistency. Rule propagation delays are an accepted tradeoff for operational simplicity and sensor autonomy:

- **Accepted stale data**: Sensors may use rules that are up to 30 seconds out of date
- **Point-in-time snapshots**: Events capture complete rule definition as evaluated
- **No hard references**: Event rule_snapshot is captured data, not database foreign key
- **Guaranteed convergence**: All sensors synchronize within configurable time window

## Explicit Non-Goals

The following features are explicitly out of scope. This is intentional to maintain simplicity.

**Version History UI**: No UI for viewing previous versions of a rule. If operators need to see what a rule looked like in the past, they query events -- the rule snapshot is embedded there.

**Rollback**: No "revert to previous version" button. Operators recreate rules manually if needed, using event snapshots as reference.

**Cross-Version Correlation**: No mechanism to correlate events across rule versions. Events are self-contained with full snapshots. Query by rule name if needed.

**Concurrent Edit Handling**: Concurrent edits result in undefined behavior. If operator A and B both edit the same rule simultaneously, one may get an error or create an orphan version. This is acceptable -- concurrent rule editing is rare and operators can refresh and retry. No optimistic locking, no conflict detection, no merge resolution.

## Edge Cases and Limitations

**Known Limitations**:

- **Retention-bounded**: Soft-deleted rules older than 28 days are permanently deleted
- **Eventual consistency**: Rule changes propagate with up to 30s delay
- **Enable/disable exception**: Boolean toggle is in-place update (not immutable)
- **No version history**: Previous versions are soft-deleted and eventually purged

**Edge Cases**:

- **Rapid edits**: Multiple edits within sync interval -- sensor gets latest on next sync
- **Concurrent edits**: Undefined behavior, may result in errors or orphan versions (acceptable)
- **Delete during testing**: Observe version deleted while events still being generated -- events have snapshot, unaffected
- **Service restart during pause**: Global pause flag resets to FALSE (rules resume automatically)

## Related Documents

**Dependencies** (read these first):

- Rule Expression Language: Defines rule schema that lifecycle controls operate on
- SDK Model: Leverages ephemeral sensor caching for rule distribution
- API Service Architecture: Documents ETAG-based conditional sync mechanism

**Related Spokes** (siblings in this hub):

- Expression Language: Defines conditions JSON structure
- Schema Evolution: Missing field handling in rule evaluation

**Extended by**:

- Event Schema and Storage: Rule snapshot embedding requirement
