---
doc_type: spoke
status: active
date_created: 2025-11-07
primary_category: architecture
hub_document: /Users/lmergen/git/trapperkeeper/doc/04-rule-engine/README.md
tags:
  - lifecycle
  - dry-run
  - operational-controls
  - rule-management
cross_cutting:
  - validation
---

# Rule Lifecycle and Operational Controls

## Context

TrapperKeeper rules run against live production data pipelines, making operational safety critical. Rules can inadvertently drop valid data, cause pipeline failures through error actions, or degrade performance. Production incidents require rapid response to disable problematic rules without permanent deletion, while new rules need validation against live traffic without impact.

**Hub Document**: This document is part of the Rule Engine Architecture. See [Rule Engine Architecture](README.md) for strategic overview and relationships to expression language, field resolution, type coercion, and schema evolution.

## Dry-Run Mode

Rules can be marked for dry-run execution, enabling production testing without impact:

**Implementation**:

- Rules have `dry_run BOOLEAN NOT NULL DEFAULT FALSE` column in database
- When `dry_run = TRUE`, API server **transforms action to "observe"** before sending to sensors
- Sensors see rule as `action: "observe"` regardless of actual configured action
- Event records capture both actual action (`"drop"`, `"error"`) and effective action (`"observe"`)

**Event schema addition**:

```json
{
  "event_id": "01936a3e-8f2a-7b3c-9d5e-123456789abc",
  "action": "observe",
  "rule": {
    "rule_id": "01936a3e-1234-7b3c-9d5e-abcdef123456",
    "action": "drop",
    "dry_run": true
  }
}
```

**Rationale**:

- Transformation happens in API layer, not SDK (keeps sensors simple)
- Sensors unaware of dry-run concept (they just execute observe actions)
- Events record actual action for analysis: "Would this rule have dropped 1000 records?"
- No performance overhead in SDK (no conditional logic in hot path)

**Web UI indication**:

- Dry-run rules display with visual badge: "DRY RUN MODE"
- Event viewer shows: "Would have dropped (dry-run)"
- Recommended workflow: Create rule → enable dry-run → validate events → disable dry-run

## Emergency Pause All Rules

Global kill switch for production incidents:

**Implementation**:

- Server maintains in-memory boolean flag: `rulesGloballyPaused`
- Flag toggled via POST `/api/admin/rules/pause` and `/api/admin/rules/resume`
- When paused, API server returns **empty rule set** with special ETAG: `"PAUSED"`
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
- Disabled rules filtered out in API query: `WHERE enabled = TRUE`
- Never sent to sensors (not included in ETAG calculation)
- Preserved in database for re-enabling later

**State transitions**:

```
CREATE → enabled=true  (default)
DISABLE → enabled=false (hidden from sensors)
ENABLE → enabled=true  (visible to sensors)
DELETE → removed from database (permanent)
```

**Web UI controls**:

- Toggle switch on rule detail page
- Bulk operations: "Disable selected rules"
- Visual indicator for disabled rules (grayed out in rule list)
- Confirmation modal: "Enable this rule? It will apply to all matching sensors in ~30 seconds"

**Rationale**:

- Safer than deletion (configuration preserved)
- No version history needed (rule still exists in database)
- Clear distinction between "temporarily disabled" and "deleted"
- Simple boolean filter in query (no performance impact)

## Concurrent Modification Handling

Last-write-wins strategy for MVP simplicity:

**Implementation**:

- Rules have `modified_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP`
- Updated automatically on every UPDATE via database trigger or ORM hook
- No optimistic locking, no version column in MVP
- Concurrent edits result in last commit overwriting previous changes

**Web UI indication**:

- Display "Last modified: 2 minutes ago by <alice@example.com>" on rule detail page
- Timestamp updates on page load (warns operator of recent changes)
- No edit conflict detection or merge resolution

**Example conflict scenario**:

1. Operator A loads rule at 10:00:00, `modified_at = 09:55:00`
2. Operator B loads same rule at 10:00:05, `modified_at = 09:55:00`
3. Operator B saves changes at 10:01:00, `modified_at = 10:01:00`
4. Operator A saves changes at 10:02:00, `modified_at = 10:02:00`
5. **Result**: Operator A's changes overwrite Operator B's changes (no conflict warning)

**Mitigation**:

- Document expected behavior in user guide
- Log all rule modifications with operator identity for incident analysis
- Display recent change timestamp prominently in UI

**Rationale**:

- Optimistic locking adds complexity for rare conflict scenario
- Team size (5 engineers) makes conflicts unlikely
- Simple timestamp tracking sufficient for debugging
- Can add version-based locking in future if conflicts become problem

## Rollback Strategy

No automated versioning; manual recovery only:

**MVP approach**:

- No rule version history table
- No "revert to previous version" button
- Deleted rules require manual re-creation

**Manual recovery options**:

1. **Database backups**: Restore from hourly/daily database snapshots
2. **Disable instead of delete**: Use enable/disable for temporary removal
3. **Audit logs**: Reconstruct rule from logged parameters (if comprehensive logging implemented)

**Rationale**:

- Version history requires significant engineering (versioned table, UI for comparing versions)
- Operators can copy rule JSON before modifications (manual backup)
- Enable/disable feature reduces need for deletion
- Accept risk for MVP, revisit if recovery incidents occur

**Future enhancement path**:

- Add `rule_versions` table with snapshot on every modification
- Web UI for browsing history and one-click revert
- Automatic snapshots before bulk operations

## Change Propagation Timeline

Rule changes propagate via eventual consistency model:

**Timeline for rule update**:

1. Operator saves rule in Web UI (t=0s)
2. Database transaction commits (t=0.05s)
3. Sensor polls API at next sync interval (t=0-30s, avg 15s)
4. Sensor receives new ETAG, fetches updated rules (t=15.1s)
5. Sensor applies updated rules to next record (t=15.1s)

**Implications**:

- No guarantee of instant propagation
- Sensors may evaluate records using stale cached rules for up to 30 seconds
- Events capture rule snapshots precisely to handle this eventual consistency
- Rule metadata in events is point-in-time data, not hard-linked to admin UI rules
- Critical updates: Reduce sync interval temporarily or restart sensors
- Emergency pause propagates at next sync (up to 30s delay)

**Design Philosophy - Eventual Consistency**:

TrapperKeeper is explicitly designed as a loosely coupled distributed system that embraces eventual consistency. Rule propagation delays are an accepted tradeoff for operational simplicity and sensor autonomy:

- **Accepted stale data**: Sensors may use rules that are up to 30 seconds out of date (configurable sync interval)
- **Point-in-time snapshots**: Events capture the complete rule definition as it existed when evaluated, preserving audit trail
- **No hard references**: Rule metadata in events is captured metadata at evaluation time, not references to admin UI rules
- **Guaranteed convergence**: All sensors synchronize within configurable time window (default 30s)
- **Operator control**: Sync interval tunable per sensor to balance freshness vs API load

This design aligns with Ephemeral Sensors principle and SDK caching model, prioritizing sensor autonomy over immediate consistency.

**Configuration**:

- Sync interval configurable per sensor via SDK initialization
- Lower interval increases API load (more frequent polls)
- Recommended default: 30 seconds (balance freshness vs overhead)

## Edge Cases and Limitations

**Known Limitations**:

- **No Conflict Prevention**: Concurrent edits result in silent overwrites
- **No Version History**: Cannot inspect previous rule states or revert changes
- **Manual Rollback**: Deleted rules require manual re-creation from backups
- **Eventual Consistency**: Rule changes propagate with up to 30s delay
- **Limited Auditability**: No built-in change history (depends on external logging)
- **In-Memory Pause State**: Global pause resets on service restart (mitigated by fail-safe default)

**Edge Cases**:

- **Multiple operators editing same rule**: Last-write-wins, no conflict detection
- **Delete during dry-run**: Rule deleted while dry-run events still being generated
- **Rapid enable/disable**: Multiple state changes within sync interval may be missed by sensors
- **Service restart during pause**: Global pause flag resets to FALSE (rules resume automatically)

## Related Documents

**Dependencies** (read these first):

- Rule Expression Language: Adds operational controls to the rule engine
- SDK Model: Leverages ephemeral sensor caching for rule distribution
- API Service Architecture: Documents ETAG-based conditional sync mechanism for rule propagation

**Related Spokes** (siblings in this hub):

- Expression Language: Defines rule schema that lifecycle controls operate on
- Schema Evolution: Dry-run mode enables testing schema evolution scenarios

**Extended by** (documents building on this):

- Event Schema and Storage: Events capture the rule that triggered them, including dry-run state
