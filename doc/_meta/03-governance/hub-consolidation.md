---
doc_type: spoke
status: active
date_created: 2025-11-06
date_updated: 2025-11-07
primary_category: documentation
hub_document: README.md
tags:
  - governance
  - hub-consolidation
  - procedures
maintainer: Documentation Team
---

# Hub Consolidation Governance

## Purpose

This document defines governance procedures for creating, maintaining, and evolving hub documents that consolidate multiple spoke documents. It establishes scope criteria, update workflows, responsibility models, and synchronization procedures to ensure hub-spoke consistency.

## Scope

**This governance applies ONLY to product documentation.**

- ✅ **Applies to**: Any .md file in `doc/` that is NOT in `doc/_meta/`
- ❌ **Does NOT apply to**: Any .md file in `doc/_meta/` (meta-documentation is exempt)

Meta-documentation uses its own organizational structure and governance, not hub-and-spoke consolidation.

## Hub Consolidation Principles

### What Is Consolidation?

**Consolidation** means creating a unified strategic overview that:

- Abstracts core concerns from implementation details
- Establishes canonical definitions used by spokes
- Explains relationships and trade-offs between approaches
- Provides narrative value beyond mere spoke inventory
- Maintains bidirectional navigation (hub↔spokes)

**Consolidation does NOT mean**:

- Duplicating implementation details from spokes
- Creating link directories without strategic context
- Replacing spokes (spokes remain authoritative for details)

### Hub vs Canonical Reference

Not every document with multiple references should become a hub.

**Hub** (consolidation of 3+ spokes):

- Multiple related documents on same architectural concern
- Strategic overview with cross-cutting relationships
- Ongoing maintenance as spokes evolve
- Example: Security Architecture Overview (6 spokes)

**Canonical Reference** (single authoritative document):

- Single comprehensive document on focused topic
- Referenced by many but not consolidating multiple sources
- No hub structure needed
- Example: UUID Strategy (single decision, widely referenced)

**Decision Algorithm**:

```
IF related_documents >= 3 AND same_concern THEN
  Create Hub
ELSE IF referenced_by >= 5 AND comprehensive THEN
  Canonical Reference (no hub needed)
ELSE
  Regular Document
END
```

## Scope Criteria

### When to Create a Hub

Create a hub when meeting **any** of these criteria:

1. **Consolidation Threshold**: 3 or more related documents on same architectural concern
2. **Size Threshold**: Single document exceeds 500 lines
3. **Reference Threshold**: Document referenced by 5+ other documents
4. **Spawning Threshold**: Document spawned 3+ specialized sub-documents

**Validation**: Retrospectively validate existing hubs against these criteria to ensure consistency.

### When NOT to Create a Hub

Do NOT create a hub when:

- Only 1-2 documents exist on topic
- Documents are unrelated despite similar keywords
- Domain is still evolving rapidly (wait for stability)
- Consolidation would obscure rather than clarify

### Spoke Definition

A **spoke** is:

- Focused document addressing specific aspect of broader concern
- References hub for strategic context
- Provides implementation details hub omits
- Maintains independence (can be read standalone)

**Spoke vs Dependency**:

- **Spoke**: Part of consolidated domain, hub provides overview
- **Dependency**: Prerequisite understanding, not consolidated by hub

## Update Propagation Workflow

### When Spoke Documents Change

When a spoke document is modified, follow this workflow:

#### 1. Author Responsibility

PR author modifying a spoke must:

- [ ] Review if changes affect hub's strategic overview
- [ ] Check if canonical definitions changed
- [ ] Update hub if consolidation section needs revision
- [ ] Document spoke changes in PR description
- [ ] Identify affected hub sections explicitly

**PR Description Template**:

```markdown
## Spoke Change Summary

**Modified Spoke**: [Document Name]
**Hub Document**: [Hub Name]
**Hub Sections Affected**: [Section numbers or "None"]

## Hub Update Required?

- [ ] Yes - canonical definitions changed
- [ ] Yes - strategic approach changed
- [ ] No - implementation detail only

## Hub Changes Included

[Describe hub updates or explain why none needed]
```

#### 2. Review Process

**Spoke-only changes** (implementation details):

- Standard PR review
- Any team member can approve
- No special hub reviewer required

**Hub-affecting changes** (canonical definitions, strategic approach):

- Dual maintainer approval required
- Spoke maintainer reviews spoke accuracy
- Hub maintainer reviews hub consolidation accuracy
- Both must approve before merge

#### 3. Hub Review Checklist

Hub maintainer verifies:

- [ ] Hub consolidation section accurately reflects spoke changes
- [ ] Cross-references remain valid
- [ ] No conflicting statements between hub and spoke
- [ ] Hub maintains strategic focus (not copying implementation details)

#### 4. Timeline

**Target**: Hub review completed within 1 sprint (2 weeks) of spoke change

**If hub update delayed**:

- Spoke PR can merge with note in PR: "Hub update tracked in issue #NNN"
- Create issue to track hub synchronization
- Hub maintainer completes update within 1 sprint

## Hub Authority Model

### Canonical Source Designation

All hub consolidation sections are designated **CANONICAL SOURCE** for strategic overview.

**Authority Hierarchy**:

1. **Hub**: Authoritative for strategic overview, canonical definitions, relationships
2. **Spoke**: Authoritative for implementation details, code examples, configuration
3. **Hub wins for strategy, Spoke wins for implementation**

### Conflict Resolution

When hub and spoke statements conflict:

#### Scenario 1: Strategic Approach Conflict

**Example**: Hub says "Use HMAC for API authentication" but spoke uses bcrypt

**Resolution**:

1. Spoke must document deviation with rationale
2. OR spoke submits PR to update hub if approach changed
3. Hub maintainer decides if hub needs update or spoke needs fix

#### Scenario 2: Canonical Definition Conflict

**Example**: Hub defines "validation occurs at 4 layers" but spoke adds 5th layer

**Resolution**:

1. Hub definition is authoritative
2. Spoke must either conform or update hub via PR
3. Hub maintainer reviews and approves definition change

#### Scenario 3: Implementation Detail Conflict

**Example**: Hub references "sqlx parameterized queries" but spoke uses different ORM

**Resolution**:

1. Spoke is authoritative for implementation
2. Hub may need update if strategic implication
3. Usually spoke just needs update

### Explicit Non-Goals

**Hub documents do NOT**:

- Replace spoke documents (spokes remain)
- Provide exhaustive implementation details (delegate to spokes)
- Lock spokes into rigid patterns (spokes can deviate with rationale)
- Eliminate all redundancy (strategic context may repeat)

## Bidirectional Reference Requirements

### Hub → Spoke References

Hubs must reference spokes using this pattern:

**In Core Concepts sections**:

```markdown
### Major Concept

[Strategic overview]

**Cross-References:**

- [Spoke Name] Section [N]: [What it specifies]
- [Spoke Name]: [What it covers]
```

**In Related Documents section**:

```markdown
**Consolidated Spokes** (this hub consolidates):

- [Spoke Name] Section [N]: Maps to this hub's Section [M]
- [Spoke Name]: Overall guidance in Section [M]
```

### Spoke → Hub References

Spokes must back-reference hub using this pattern:

**In Context section**:

```markdown
## Context

[Spoke-specific context]

**Hub Document**: This document is part of the [Hub Name] architecture.
See [Hub Document Link] for strategic overview and relationships to other components.

[Continue with spoke content]
```

### Automated Validation

Bidirectional references are enforced via CI/CD:

```bash
python doc/scripts/analyzer.py validate-hub-spoke-links
```

**Checks**:

1. All hubs list spokes in Related Documents section
2. All listed spokes exist
3. All spokes include hub back-reference
4. No orphaned spokes (spoke references hub that doesn't list it)

**Merge Gate**: PR cannot merge if validation fails

## Hub Maintenance Procedures

### Adding a New Spoke

When creating a new document that fits hub's domain:

#### 1. Author Actions

- [ ] Write spoke document following standard structure
- [ ] Add hub back-reference in Context section
- [ ] Add to hub's Related Documents section
- [ ] Add to relevant Core Concepts subsection in hub
- [ ] Verify 3-click navigation maintained

#### 2. Hub Maintainer Review

- [ ] Verify spoke fits hub's domain scope
- [ ] Check spoke placement in Core Concepts appropriate
- [ ] Ensure no duplicate coverage with existing spokes
- [ ] Validate cross-references
- [ ] Approve PR

#### 3. Template PR

```markdown
## Summary

Adding [Spoke Name] to [Hub Name] architecture

## Changes

- Created spoke document at [path]
- Added hub back-reference in spoke Context section
- Updated hub Related Documents section
- Added spoke to hub Section [N] with cross-reference

## Checklist

- [ ] Spoke includes hub back-reference
- [ ] Hub lists spoke in Related Documents
- [ ] Hub Core Concepts section links to spoke
- [ ] 3-click navigation verified
- [ ] Bidirectional validation passes
```

### Updating a Spoke

See "Update Propagation Workflow" section above.

### Removing or Deprecating a Spoke

When a spoke is superseded, merged, or deprecated:

#### 1. Author Actions

- [ ] Update spoke status to 'superseded' or 'deprecated'
- [ ] Add superseded_by field if applicable
- [ ] Remove from hub's Related Documents section
- [ ] Remove from hub's Core Concepts cross-references
- [ ] Verify no broken links remain

#### 2. Hub Maintainer Review

- [ ] Verify removal justified
- [ ] Check if hub structure needs adjustment
- [ ] Ensure replacement spoke listed if applicable
- [ ] Validate no broken references
- [ ] Approve PR

### Splitting a Hub

When a hub exceeds 10 spokes or 1000 lines, consider splitting:

#### 1. Identify Natural Boundaries

Look for:

- Distinct sub-domains within hub
- Groups of 3-5 spokes forming cohesive cluster
- Minimal cross-references between groups

#### 2. Create Sub-Hubs

- Each sub-hub consolidates 3-5 spokes
- Original hub becomes "super-hub" linking sub-hubs
- OR original hub deprecated, sub-hubs stand alone

#### 3. Update All References

- Spokes update hub back-references to sub-hubs
- Cross-cutting indexes updated
- Navigation paths verified

## Quarterly Hub Maintenance

### Review Schedule

Conduct quarterly review of each hub on rotating schedule:

**Q1 (Jan-Mar)**: Security, Validation
**Q2 (Apr-Jun)**: Performance, Error Handling
**Q3 (Jul-Sep)**: [Other hubs]
**Q4 (Oct-Dec)**: All hubs comprehensive review

### Review Checklist

For each hub under review:

- [ ] Verify all consolidated spokes still exist
- [ ] Check for broken cross-references
- [ ] Run `analyzer.py validate-hub-spoke-links`
- [ ] Assess if new spokes should be added
- [ ] Evaluate if hub structure needs refinement
- [ ] Confirm 3-click navigation metric satisfied
- [ ] Review if any spokes should be removed
- [ ] Check for conflicts between hub and spokes

### Consolidation Accuracy Checklist

Verify hub consolidation quality:

- [ ] Hub provides strategic overview (not implementation copy)
- [ ] Canonical definitions accurate and current
- [ ] Relationships between concepts explained
- [ ] Cross-references include section numbers
- [ ] No duplicate content between hub and spokes
- [ ] Examples illustrate concepts (not duplicate spoke examples)
- [ ] Trade-offs and constraints documented

### Time Estimate

30-60 minutes per hub for quarterly review

## Responsibility Matrix (Lightweight RACI)

For a 5-person team, responsibilities are lightweight and flexible:

### Hub Creation

- **Responsible**: Person proposing hub (author)
- **Approves**: Team consensus (any 2 team members)
- **Informed**: Whole team via PR

### Hub Content Updates

- **Responsible**: Hub maintainer (designated in hub frontmatter)
- **Approves**: Hub maintainer + spoke author (for hub-affecting spoke changes)
- **Consulted**: Domain experts as needed
- **Informed**: Team via PR

### Spoke Addition/Removal

- **Responsible**: Spoke author + hub maintainer
- **Approves**: Hub maintainer
- **Informed**: Team via PR

### Quarterly Review

- **Responsible**: Hub maintainer
- **Approves**: Any team member (review PR)
- **Informed**: Team via PR with "Quarterly review YYYY-QN"

### Conflict Resolution

- **Responsible**: Hub maintainer (first attempt)
- **Escalates to**: Team discussion if maintainer can't resolve
- **Decides**: Team consensus

**Note**: For 5-person team, formal RACI is overkill. This lightweight model provides clarity without bureaucracy.

## Version Alignment

### Hub-Spoke Synchronization

**Approach**: Timestamp-based synchronization (simplest for 5-person team)

Hub frontmatter includes:

```yaml
date_updated: 2025-11-06
```

Spokes include:

```yaml
date_updated: 2025-11-05
hub_document: doc/security/README.md
```

**Synchronization Check**:

```bash
python doc/scripts/analyzer.py check-hub-freshness
```

Reports hubs where spokes updated more recently than hub (possible drift).

**No Semantic Versioning**: For documentation, timestamps sufficient. Semantic versioning adds complexity without clear benefit for 5-person team.

## Hub Growth Management

### When Hub Becomes Too Large

**Thresholds**:

- More than 10 spokes
- Document exceeds 1000 lines
- Takes >10 minutes to read hub overview

**Options**:

#### Option 1: Split into Sub-Hubs

Create 2-3 sub-hubs, each consolidating 3-5 spokes on related sub-domains.

**Example**: Security Architecture Hub splits into:

- Authentication and Authorization Hub
- Encryption and Transport Security Hub
- Input Sanitization and Validation Hub (potentially moves to Validation Hub)

#### Option 2: Promote Spokes

Some spokes become independent canonical references if widely referenced but not needing consolidation.

#### Option 3: Archive Deprecated Content

Move outdated spokes to archive, remove from hub active consolidation.

### Hub Consolidation Should Not

- Exceed 1000 lines in main document (use appendices)
- List more than 10 spokes (split into sub-hubs)
- Take >10 minutes to read overview sections
- Duplicate extensive code examples from spokes

## Existing Hub Validation

Apply scope criteria retrospectively to existing hubs:

**Current Formal Hubs**:

- Unified Validation and Input Sanitization (8 spokes) ✓
- Security Architecture Overview (6+ spokes) ✓
- Performance Model and Optimization Strategy (multiple spokes) ✓
- Error Handling Strategy (multiple spokes) ✓

**Validation**:

- All meet ≥3 spoke consolidation threshold ✓
- All provide strategic overview ✓
- All maintain spoke references ✓
- Consider whether any should split (unlikely given current sizes)

## Related Documents

- **Standards**: `doc/_meta/standards/hub-and-spoke-architecture.md` - Hub structure requirements
- **Template**: `doc/_meta/templates/hub.md` - Canonical hub template
- **Automation**: `doc/_meta/tooling/architecture.md` - analyzer.py design
