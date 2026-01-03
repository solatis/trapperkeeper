---
doc_type: spoke
status: active
date_updated: 2025-11-07
primary_category: documentation
hub_document: doc/_meta/03-governance/README.md
tags:
  - governance
  - cross-cutting
  - indexes
maintainer: Documentation Team
---

# Cross-Cutting Index Governance

**Related Documents**:

- **Template**: `doc/_meta/templates/cross-cutting-index.md` - Canonical structure with inline guidance
- **Frontmatter Schema**: `doc/_meta/standards/frontmatter-reference.md` - Required metadata fields for indexes

## Purpose

This document defines governance, ownership, and maintenance procedures for cross-cutting indexes in Trapperkeeper documentation. Cross-cutting indexes provide navigation entry points for concerns that span multiple domains (security, performance, validation, observability, error handling).

## What Are Cross-Cutting Indexes?

Cross-cutting indexes organize documentation around concerns that affect multiple architectural domains. Unlike hub documents (which consolidate a single domain), indexes provide discovery mechanisms across domain boundaries.

**Standard Cross-Cutting Concerns**:

- **Security**: Authentication, authorization, encryption, threat models, compliance
- **Performance**: Latency, throughput, resource limits, optimization strategies, cost models
- **Validation**: Input sanitization, type checking, schema validation, error handling
- **Observability**: Logging, metrics, tracing, debugging, operational visibility
- **Error Handling**: Error classification, recovery strategies, user messaging, failure modes

## Index Structure

All cross-cutting indexes must follow the canonical template at `doc/_meta/templates/cross-cutting-index.md`.

### Required Sections

1. **Frontmatter**: Metadata with doc_type='index', maintainer, review dates
2. **Purpose**: 2-3 sentences explaining the concern and audience
3. **Quick Reference**: Table with 3-5 high-level categories
4. **Core Concepts**: 3-7 subsections organizing documentation by aspect
5. **Domain Coverage Matrix**: Shows which domains address the concern
6. **Patterns and Best Practices**: Recurring patterns with references
7. **Related Indexes**: Links to overlapping concerns
8. **Maintenance Notes**: Review schedule and known gaps

### Content Organization Patterns

**Hierarchical Organization** (preferred for most concerns):

```
Core Concepts
  └─ Aspect 1
       └─ Hub Document → Spoke Document → Section
  └─ Aspect 2
       └─ Hub Document → Spoke Document → Section
```

**Domain-Oriented Organization** (when concern varies significantly by domain):

```
Domain Coverage Matrix
  └─ Domain 1 → Key Document
  └─ Domain 2 → Key Document
```

**Pattern-Oriented Organization** (when repeating patterns exist):

```
Patterns and Best Practices
  └─ Pattern 1 → Used In [Doc A, Doc B]
  └─ Pattern 2 → Used In [Doc C, Doc D]
```

## Ownership Model

### Primary Maintainer

Each cross-cutting index must have a designated **primary maintainer** specified in frontmatter.

**Responsibilities**:

- Conduct quarterly reviews
- Update index when new documentation added
- Resolve conflicts when documents could appear in multiple categories
- Track known gaps and planned additions
- Coordinate with domain owners

**For Trapperkeeper** (5-person team):

- Maintainer is typically the person most familiar with the concern
- No formal rotation required due to small team size
- Maintainer documented in index frontmatter

### Document Authors

Authors who add new documentation addressing a cross-cutting concern must:

1. Add `cross_cutting: [concern-name]` to document frontmatter
2. Update relevant cross-cutting index to link to new document
3. Place entry in appropriate section/category
4. Update Domain Coverage Matrix if new domain

## Update Triggers

Cross-cutting indexes must be updated when:

### 1. New Document Created

**When**: New hub or spoke document published with cross-cutting concern

**Action**:

- Author adds `cross_cutting: [concern]` to frontmatter
- Author updates relevant index with link
- Author places in appropriate Core Concepts subsection
- Author updates Domain Coverage Matrix if new domain

**Review**: Primary maintainer reviews placement during PR

### 2. Document Significantly Updated

**When**: Major revision affecting cross-cutting concern scope

**Action**:

- Author updates index if concern scope changed
- Primary maintainer verifies during quarterly review

### 3. New Pattern Identified

**When**: Recurring pattern recognized across 3+ documents

**Action**:

- Pattern added to "Patterns and Best Practices" section
- Documents using pattern cross-referenced
- Primary maintainer approves pattern addition

### 4. Quarterly Review

**When**: Every 3 months (see schedule below)

**Action**:

- Verify all links functional
- Check for new documents missing from index
- Update Domain Coverage Matrix
- Document known gaps
- Update review dates in frontmatter

### 5. Domain Restructuring

**When**: Hub consolidation or major documentation reorganization

**Action**:

- Primary maintainer updates all affected index entries
- Verifies no broken links
- Updates Quick Reference table if categories changed

## Approval Process

### Regular Updates

**Scope**: Adding new document link, updating existing link, minor corrections

**Process**:

1. Author creates PR with documentation + index update
2. Primary maintainer reviews index changes
3. Any team member can approve PR

**Timeline**: Same as normal PR review (no special SLA)

### Structural Changes

**Scope**: Changing Quick Reference categories, adding new concern, reorganizing Core Concepts sections

**Process**:

1. Author creates PR with rationale for structural change
2. Primary maintainer must approve
3. Team discussion if reorganization is significant
4. Update template if pattern should apply to all indexes

**Timeline**: Allow 2-3 days for team input on significant changes

## Conflict Resolution

### When Documents Fit Multiple Categories

**Scenario**: Document addresses a cross-cutting concern in multiple aspects

**Resolution**:

- Primary entry in most relevant aspect
- Brief mention with cross-reference in other aspects
- Pattern: "See [Aspect X] for primary discussion of [Document]"

**Example**:

```markdown
### Authentication Strategy

**Relevant Documentation:**

- **Security Architecture Overview** - Comprehensive authentication design

### Transport Security

**Relevant Documentation:**

- **Security Architecture Overview** Section 3 - TLS configuration for authentication
```

### When Multiple Indexes Could Include Document

**Scenario**: Document addresses multiple cross-cutting concerns

**Resolution**:

- Document includes all concerns in frontmatter: `cross_cutting: [security, performance]`
- Document appears in all relevant indexes
- Each index highlights different aspect

**Example**:

- Security Index: Focuses on validation for input sanitization
- Performance Index: Focuses on validation cost and optimization

### When Categorization Is Unclear

**Scenario**: Unclear which Quick Reference category document belongs to

**Resolution**:

1. Primary maintainer makes initial decision
2. Document in Maintenance Notes if categorization debated
3. Revisit during quarterly review
4. Move if better categorization identified

## Automation Strategy

### Automated Index Generation

**Tool**: `python doc/scripts/analyzer.py generate-index --concern [concern-name]`

**When to Run**:

- Pre-commit hook (optional, can be slow)
- CI/CD validation on pull requests
- Manual execution during quarterly review

**How It Works**:

1. Scans all documents with `cross_cutting: [concern]` in frontmatter
2. Groups by primary_category for Domain Coverage Matrix
3. Extracts document titles and descriptions
4. Generates baseline index structure

**Manual Customization**:

- Automation generates baseline structure
- Primary maintainer adds narrative, patterns, categorization
- Manual additions preserved in subsequent runs via YAML markers

**Preservation Pattern**:

```markdown
<!-- BEGIN MANUAL SECTION -->

[Hand-written content preserved during automation]

<!-- END MANUAL SECTION -->
```

### Automated Validation

**Tool**: `python doc/scripts/validate.py validate-indexes`

**Checks**:

1. All documents with `cross_cutting` frontmatter appear in corresponding index
2. All index links resolve to existing documents
3. No duplicate entries within same index
4. Frontmatter includes required fields
5. Review dates not expired (>3 months old)

**When to Run**:

- CI/CD validation on every PR
- Monthly automated check with notification

## Quarterly Review Process

Cross-cutting indexes follow a rotating quarterly review schedule:

**Schedule**:

- Q1 (Jan-Mar): Security Index, Validation Index
- Q2 (Apr-Jun): Performance Index, Observability Index
- Q3 (Jul-Sep): Error Handling Index, [Future Indexes]
- Q4 (Oct-Dec): All indexes comprehensive review

**Review Checklist**:

- [ ] Run `validate.py validate-indexes` and fix issues
- [ ] Verify all links functional (automated + manual spot-check)
- [ ] Check for new documents with cross_cutting frontmatter missing from index
- [ ] Review Domain Coverage Matrix for completeness
- [ ] Identify and document known gaps
- [ ] Update patterns if new patterns identified
- [ ] Update last_review and next_review dates in frontmatter
- [ ] Create PR with "Quarterly review YYYY-QN" commit message

**Time Estimate**: 30-60 minutes per index

**Responsible**: Primary maintainer (designated in index frontmatter)

## Adding New Cross-Cutting Concerns

**Threshold for New Index**:

- Concern spans 5+ documents across 3+ domains
- Concern not adequately covered by existing indexes
- Team consensus that concern warrants dedicated navigation

**Process**:

1. Propose new concern with rationale
2. Team discusses need and scope
3. Designate primary maintainer
4. Create index using canonical template
5. Add to quarterly review rotation
6. Update this governance document

**Current Concerns**: security, performance, validation, observability, error-handling

## Link Format Standards

### Descriptive Link Text

**Preferred**: Use descriptive text explaining what document covers

```markdown
- **Unified Validation Strategy** - Complete validation architecture across 4 layers
```

**Avoid**: Generic references without context

```markdown
- See document 030
- Validation document
```

### Section-Specific Links

When pointing to specific content within documents:

**Pattern**: `[Document Name] Section [N] - [What that section covers]`

**Example**:

```markdown
- **Security Architecture Overview** Section 2.1 - Web UI authentication with bcrypt
```

### Deep Linking with Anchors

For precise navigation to subsections:

**Pattern**: `[Document Name](#anchor-id) - [Specific topic]`

**Example**:

```markdown
- **Validation Strategy**(#input-sanitization) - OWASP SQL injection prevention
```

## Drift Detection

**Drift** occurs when indexes become stale or inaccurate.

**Detection Mechanisms**:

1. **Automated CI**: Flags missing cross_cutting documents
2. **Quarterly Review**: Manual verification of completeness
3. **PR Reviews**: Maintainer catches missing index updates
4. **Monthly Report**: Automated summary of potential drift

**Prevention**:

- Make index updates part of PR checklist
- Automated validation in CI/CD
- Clear ownership per index
- Quarterly review cadence

## Relationship to Hub Documents

Cross-cutting indexes and hub documents serve different purposes:

| Aspect       | Cross-Cutting Index       | Hub Document                |
| ------------ | ------------------------- | --------------------------- |
| Purpose      | Navigation across domains | Consolidation within domain |
| Organization | By cross-cutting concern  | By architectural domain     |
| Scope        | Spans multiple domains    | Single domain (3+ spokes)   |
| Content      | Links with brief context  | Full strategic narrative    |
| Maintenance  | Quarterly review          | Updated with spoke changes  |

**Complementary Pattern**:

- Hub documents provide depth within a domain
- Cross-cutting indexes provide breadth across domains
- Both link to the same spoke documents from different perspectives

**Example**:

- **Security Hub**: Deep dive into authentication, encryption, threat model
- **Performance Index**: Links to Security Hub Section on HMAC vs bcrypt performance trade-off

## Related Documents

- **Template**: `doc/_meta/templates/cross-cutting-index.md` - Canonical index structure
- **Standards**: `doc/_meta/standards/hub-and-spoke-architecture.md` - Hub document requirements
- **Automation**: `doc/_meta/tooling/architecture.md` - analyzer.py design
