# ADR-018: Rule Lifecycle and Operational Controls
Date: 2025-10-28

## Context

TrapperKeeper rules run against live production data pipelines, making operational safety critical. Rules can inadvertently drop valid data, cause pipeline failures through error actions, or degrade performance. Production incidents require rapid response to disable problematic rules without permanent deletion.

Key operational challenges:

- **Testing in production**: No separate staging environment in MVP; rules must be validated against live traffic
- **Emergency response**: Critical incidents require immediate rule deactivation
- **Concurrent modifications**: Multiple operators may edit rules simultaneously in Web UI
- **Change tracking**: Operators need visibility into recent rule changes for incident investigation
- **Rollback requirements**: Must recover from accidental deletions or incorrect modifications
- **Zero downtime**: Rule updates propagate to sensors without service restarts

Constraints from distributed architecture (ADR-001, ADR-004):

- Sensors cache rules in memory with periodic sync (default: 30 seconds)
- Rules propagate via ETAG-based polling (pull model, not push)
- No persistent connections between sensors and API server
- Eventual consistency acceptable (sync interval determines propagation delay)

## Decision

We will implement **lifecycle controls for safe rule management** with dry-run testing, emergency controls, and simple concurrency handling.

### 1. Dry-Run Mode

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

### 2. Emergency Pause All Rules

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

### 3. Individual Rule Enable/Disable

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

### 4. Concurrent Modification Handling

Last-write-wins strategy for MVP simplicity:

**Implementation**:
- Rules have `modified_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP`
- Updated automatically on every UPDATE via database trigger or ORM hook
- No optimistic locking, no version column in MVP
- Concurrent edits result in last commit overwriting previous changes

**Web UI indication**:
- Display "Last modified: 2 minutes ago by alice@example.com" on rule detail page
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

### 5. Rollback Strategy

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

### 6. Change Propagation Timeline

Rule changes propagate via eventual consistency model:

**Timeline for rule update**:
1. Operator saves rule in Web UI (t=0s)
2. Database transaction commits (t=0.05s)
3. Sensor polls API at next sync interval (t=0-30s, avg 15s)
4. Sensor receives new ETAG, fetches updated rules (t=15.1s)
5. Sensor applies updated rules to next record (t=15.1s)

**Implications**:
- No guarantee of instant propagation
- In-flight records may use old rules (acceptable for observability use case)
- Critical updates: Reduce sync interval temporarily or restart sensors
- Emergency pause propagates at next sync (up to 30s delay)

**Configuration**:
- Sync interval configurable per sensor via SDK initialization
- Lower interval increases API load (more frequent polls)
- Recommended default: 30 seconds (balance freshness vs overhead)

## Consequences

### Benefits

1. **Production Testing**: Dry-run mode enables validation against live traffic without impact
2. **Emergency Response**: Global pause provides instant mitigation for critical incidents
3. **Soft Deletion**: Enable/disable preserves configuration while hiding rules from sensors
4. **Implementation Simplicity**: Last-write-wins avoids complex locking or conflict resolution
5. **Clear Expectations**: Timestamp tracking provides visibility into recent changes
6. **Fast Recovery**: Global pause stored in memory (no database latency)
7. **Safe Defaults**: Disabled and paused states prevent unintended rule execution

### Tradeoffs

1. **No Conflict Prevention**: Concurrent edits result in silent overwrites
2. **No Version History**: Cannot inspect previous rule states or revert changes
3. **Manual Rollback**: Deleted rules require manual re-creation from backups
4. **Eventual Consistency**: Rule changes propagate with up to 30s delay
5. **Limited Auditability**: No built-in change history (depends on external logging)
6. **In-Memory Pause State**: Global pause resets on service restart (mitigated by fail-safe default)

### Operational Implications

1. **Testing Workflow**: Operators should always enable dry-run for new rules initially
2. **Incident Response**: Use global pause first, then investigate and disable specific problematic rules
3. **Change Coordination**: Operators should communicate before editing shared rules (out-of-band coordination)
4. **Backup Discipline**: Copy rule JSON before major modifications (manual safety net)
5. **Monitoring**: Track pause state and disabled rule count in dashboards

## Implementation

1. **Database schema changes**:
   ```sql
   ALTER TABLE rules ADD COLUMN enabled BOOLEAN NOT NULL DEFAULT TRUE;
   ALTER TABLE rules ADD COLUMN dry_run BOOLEAN NOT NULL DEFAULT FALSE;
   ALTER TABLE rules ADD COLUMN modified_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP;
   CREATE INDEX idx_rules_enabled ON rules(enabled) WHERE enabled = TRUE;
   ```

2. **API layer modifications**:
   - Add global pause state management (in-memory flag)
   - Filter rules by `enabled = TRUE` in query
   - Transform `action` to `"observe"` when `dry_run = TRUE`
   - Include `paused` field in sync response

3. **Admin endpoints**:
   - `POST /api/admin/rules/pause` - Set global pause flag
   - `POST /api/admin/rules/resume` - Clear global pause flag
   - `GET /api/admin/rules/status` - Return current pause state

4. **Web UI enhancements**:
   - Toggle switch for enable/disable on rule detail page
   - Checkbox for dry-run mode with warning text
   - Global pause button in top navigation (prominent red button)
   - Banner when globally paused: "⚠️ ALL RULES PAUSED"
   - Display `modified_at` timestamp on rule list and detail pages

5. **Logging and observability**:
   - Log all pause/resume events with operator identity
   - Log all enable/disable actions with rule_id and operator
   - Expose Prometheus metrics:
     - `trapperkeeper_rules_globally_paused` (gauge)
     - `trapperkeeper_rules_disabled_total` (gauge)
     - `trapperkeeper_rules_dryrun_total` (gauge)

6. **SDK changes** (minimal):
   - No SDK changes required (dry-run transformation in API layer)
   - Empty rule set handled by existing logic (no rules = no actions)

7. **Documentation updates**:
   - Document last-write-wins concurrency model
   - Provide recommended testing workflow (dry-run → validate → enable)
   - Document emergency response procedure (pause → investigate → disable specific rules)
   - Warn about lack of automatic version history

## Related Decisions

**Depends on:**
- **ADR-014: Rule Expression Language** - Adds operational controls to the rule engine
- **ADR-002: SDK Model** - Leverages ephemeral sensor caching for rule distribution

**Related:**
- **ADR-004: API Service Architecture** - Documents ETAG-based conditional sync mechanism
- **ADR-013: Event Schema** - Documents event storage that captures dry-run actions

## Future Considerations

- **Optimistic locking**: Add `version` column for conflict detection, return 409 Conflict on stale updates
- **Version history**: Store rule snapshots in `rule_versions` table on every modification
- **Audit trail**: Comprehensive logging of all rule changes with operator identity, timestamp, and diff
- **Scheduled changes**: Future-dated rule activations (enable rule at specific time)
- **Staged rollouts**: Percentage-based deployment (apply rule to 10% of sensors, gradually increase)
- **Change approval workflow**: Require review before rule activation (especially for "error" and "drop" actions)
- **Diff view**: Compare current rule with previous version before saving
- **Bulk operations**: Enable/disable/dry-run for multiple rules simultaneously
- **Rule templates**: Save common patterns for reuse, reducing re-creation effort
