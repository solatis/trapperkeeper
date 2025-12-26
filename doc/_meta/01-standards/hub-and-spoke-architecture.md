---
doc_type: spoke
status: active
date_created: 2025-11-06
date_updated: 2025-11-07
primary_category: documentation
hub_document: README.md
tags:
  - standards
  - architecture
  - hub-spoke
maintainer: Documentation Team
---

# Hub and Spoke Documentation Architecture

## Purpose

This document defines the hub-and-spoke architecture pattern for technical documentation in Trapperkeeper. It establishes mandatory structure requirements, navigation standards, and quality criteria for hub documents that consolidate related concerns.

**CRITICAL SCOPE**: This standard ONLY applies to product documentation in `doc/` (excluding `doc/_meta/`).
NEVER apply hub-and-spoke patterns to meta-documentation.

## Scope

**Applies to**: `doc/**/*.md` (excluding `doc/_meta/`)
**Does NOT apply**: `doc/_meta/**/*.md` (uses separate structure: standards/, templates/, governance/, tooling/)

## Overview

Hub-and-spoke architecture organizes complex, multi-faceted technical domains into:

- **Hub documents**: Consolidate fragmented decisions across related domains, providing unified strategic overview, cross-cutting concerns, canonical definitions, and navigation to detailed implementations
- **Spoke documents**: Provide implementation specifics within focused domains with detailed technical specifications, code examples, and references back to hub for strategic context

This pattern delivers:

- **Bidirectional navigation**: Big picture to specifics, specifics to strategy
- **Single source of truth**: Canonical definitions in hubs
- **Cohesion**: Logical grouping while maintaining readability
- **Simplified maintenance**: Cross-cutting updates in one place

## When to Create a Hub

Create a hub when a domain meets ANY of these criteria:

1. **Consolidation threshold**: 3+ related documents addressing the same architectural concern
2. **Size threshold**: Single document exceeds 500 lines
3. **Reference threshold**: Document referenced by 5+ other documents
4. **Spawning threshold**: Document spawned 3+ specialized sub-documents

**Target Size**: 3-10 consolidated spokes per hub
**Maximum Limits**: 10 spokes OR 1000 lines (whichever comes first)
**Action Required**: Exceeding limits REQUIRES splitting into multiple hubs

**Example**: A "State Management" hub consolidating Redux, MobX, and Context docs (3 spokes) is optimal. A hub with 15 framework integrations must split (e.g., "Client State Management" and "Server State Management" hubs).

## Hub Naming Convention

**MANDATORY**: All hub documents MUST be named `README.md`.

**Rationale**:

- **Discoverability**: GitHub and GitLab platforms provide special rendering for README.md files, making them immediately visible when browsing directories
- **Uniqueness enforcement**: The README.md convention naturally enforces the "single hub per directory" architectural constraint since only one README.md can exist per directory
- **Consistency**: Establishes a uniform pattern across the entire project, reducing cognitive load when navigating documentation

**Examples**:

✅ CORRECT: `doc/security/README.md` (hub document)
✅ CORRECT: `doc/performance/README.md` (hub document)
❌ WRONG: `doc/security/overview.md` (use README.md instead)
❌ WRONG: `doc/security/index.md` (use README.md instead)
❌ WRONG: `doc/security/security-hub.md` (use README.md instead)

**Directory Structure Pattern**:

```
doc/security/
├── README.md              # Hub document (consolidates security concerns)
├── authentication.md      # Spoke document
├── authorization.md       # Spoke document
└── tls-configuration.md   # Spoke document
```

This naming convention applies exclusively to documents with `doc_type: hub` in their frontmatter. Spoke documents, guides, and reference materials may use descriptive filenames.

## Hub Document Structure

All hub documents MUST follow this canonical structure:

### Required Sections

#### 1. Title and Metadata

```markdown
# [Domain Name] Architecture
```

**FORBIDDEN PATTERNS**:

- NEVER include revision logs, version history, or change tracking sections
- NEVER include "Last Updated" timestamps or author attribution
- NEVER embed document meta-discussion in the content itself

**Rationale**: Git is the authoritative source for document history. In-document tracking creates maintenance burden and version conflicts.

#### 2. Context

State the problem being solved in 2-4 paragraphs:

- What fragmentation currently exists?
- What pain points does this create?
- Why is consolidation necessary?

Must answer: "Why does this hub exist?"

#### 3. Decision

Provide strategic overview in 3-5 paragraphs:

- High-level approach chosen
- Core principles and constraints
- How this addresses the context
- Explicit statement that this is a hub document

Must answer: "What is the unified strategy?"

#### 4. Core Concepts (Subsections)

Break down the domain into 3-7 major subsections. Each subsection MUST follow this exact pattern (deviations require explicit justification in PR):

**REQUIRED elements** (all subsections MUST include):

- Strategic overview (2-3 paragraphs)
- Key Points list (3-5 bullets)
- Cross-References section (minimum 1 spoke reference)

**OPTIONAL elements**:

- Example block (recommended for abstract concepts)
- Diagram references (when visual aids clarify relationships)

**Subsection Template**:

```markdown
### N. Concept Name

[2-3 paragraphs of strategic overview]

**Key Points:**

- [Essential concept 1]
- [Essential concept 2]
- [Essential concept 3]

**Cross-References:**

- [Document Name]: [What it specifies]
- [Document Name]: [What it specifies]

**Example**: [optional 1-2 sentence concrete example]
```

**Cross-Reference Format**:

✅ CORRECT: `[Query Optimization]: Performance characteristics and index strategies`
✅ CORRECT: `[Connection Pooling]: Resource management and scaling considerations`
❌ WRONG: `[Document 4]: See this document`
❌ WRONG: `[Link]: Details here`

The reference MUST state WHY a reader would navigate to the spoke (what specific information they'll find).

#### 5. Related Documents

List all consolidated spoke documents with bidirectional mapping:

```markdown
## Related Documents

**Consolidated Spokes** (this hub consolidates):

- [Document Name]: [Which sections map to this hub]
- [Document Name]: [Which sections map to this hub]

**Dependencies** (foundational documents):

- [Document Name]: [Why this is foundational]

**References** (related hubs/documents):

- [Document Name]: [How they relate]
```

### Optional Sections

- **Appendices**: Supporting reference material not essential to main narrative
- **Examples**: Extended worked examples
- **Migration Notes**: Guidance on transitioning from old patterns

## Hub Quality Criteria

### Structural Requirements

- [ ] Document starts with title (no revision log)
- [ ] Context section exists and explains fragmentation problem
- [ ] Decision section explicitly states this is a hub document
- [ ] Contains 3-7 core concept subsections
- [ ] Related Documents section lists all consolidated spokes
- [ ] Cross-references use descriptive text, not document numbers alone

### Navigational Requirements

- [ ] Maximum 3 clicks from hub to any spoke detail
- [ ] Back-reference compliance: 100% (all spokes must include hub reference)
  - **Measurement**: Count spokes with "Hub Document:" back-reference / total spokes
  - **Required**: 10/10 spokes reference hub (100%)
  - **Unacceptable**: 9/10 spokes reference hub (90%) - PR blocked
  - **Action**: Missing back-references must be added before merge
- [ ] Bidirectional linking: hub→spokes and spokes→hub
- [ ] Each subsection includes explicit cross-references
- [ ] CRITICAL: Zero orphaned spokes permitted (unlinked from hub)
  - **Detection**: Search codebase for spoke documents not referenced in any hub
  - **Consequence**: Orphaned spokes indicate architectural debt or missing hub
  - **Action**: Either link to existing hub or create new hub to consolidate

### Content Requirements

- [ ] Provides narrative context beyond spoke inventory
- [ ] Consolidates rather than duplicates spoke content
- [ ] Explains relationships between concepts
- [ ] Uses domain language consistently
- [ ] Includes concrete examples for abstract concepts
- [ ] Avoids implementation details (delegate to spokes)

### Consolidation Semantics

**Consolidation means**:

- Hub abstracts core concerns while spokes provide implementation detail
- Hub provides narrative value beyond mere spoke inventory
- Hub establishes canonical definitions used by spokes
- Hub explains relationships and trade-offs between approaches

**FORBIDDEN HUB ANTI-PATTERNS**:

❌ **The Link Farm**: List of spoke links with no explanatory context
Example: "See: Doc1, Doc2, Doc3, Doc4" (no narrative, no relationships)

❌ **The Copy-Paste Hub**: Duplicating spoke implementation details
Example: Including full code examples already present in spokes

❌ **The Spoke Replacement**: Attempting to make hub self-sufficient
Example: Hub tries to answer all questions without delegating to spokes

❌ **The Index Pretender**: Navigation structure masquerading as strategic document
Example: Hierarchical bullet list with no prose, no insights, no trade-off analysis

**DETECTION**: If removing all cross-reference links makes the hub useless, it's a link farm. Hubs must provide narrative value independently of navigation.

## 3-Click Navigation Metric

The "3-click navigation" success criterion means:

**Definition**: From any hub document, a reader can reach detailed implementation guidance in 3 or fewer clicks (hyperlinks).

**Measurement**:

1. Start at hub document
2. Count clicks through cross-references to reach implementation detail
3. Path must not exceed 3 clicks

**Examples**:

✅ Valid: Hub → Core Concept Section → Spoke Document cross-reference → Specific Implementation Section = 2 clicks
✅ Valid: Hub subsection → Spoke anchor link = 1 click (optimal)
✅ Valid: Hub → Spoke → Sub-spoke = 2 clicks (acceptable for deep hierarchies)
❌ Invalid: Hub → Index → Category → Spoke → Section = 4 clicks
❌ Invalid: Hub → TOC page → Category → Spoke = 3 clicks (unnecessary intermediate page)
❌ Invalid: Hub → "See related docs" → Index → Spoke = 3 clicks (poor information architecture)

**RULE**: Every intermediate page in the navigation path MUST add semantic value. Pure navigation pages (indexes, TOCs without content) count against the click budget without providing value.

## Spoke Back-Reference Pattern

All spoke documents MUST include a reference back to their hub using this pattern:

```markdown
## Context

[Spoke-specific context explaining this document's focused scope]

**Hub Document**: This document is part of the [Hub Name] architecture. See [Hub Document Link] for strategic overview and relationships to other components.

[Continue with spoke-specific content]
```

This pattern:

- Appears in the Context section
- Uses "Hub Document" as the label
- Links to the specific hub document
- Orients readers on the big picture before diving into details

**Relationship to Frontmatter Field**: This markdown pattern provides human-readable context for readers. Spoke documents MUST also include the `hub_document` frontmatter field for machine-readable validation (see `frontmatter-reference.md`). These patterns are complementary:

- **Markdown `**Hub Document**:`**: Human-readable prose in Context section explaining strategic relationship and orienting readers
- **Frontmatter `hub_document`**: Machine-readable metadata enabling validation tooling, automated spoke discovery, and relationship verification

Both patterns serve different audiences (human readers vs. automated tools) and MUST be present in all spoke documents. The 90% back-reference compliance metric applies to the markdown pattern's presence in spoke Context sections.

## Hub Maintenance Procedures

### Adding a New Spoke

**WORKFLOW: Adding a New Spoke**

**Prerequisites**:

- [ ] New spoke document written and reviewed
- [ ] Spoke's domain clearly maps to existing hub

**Steps** (all steps REQUIRED):

1. **Update hub "Related Documents"**:
   - Add spoke to "Consolidated Spokes" list
   - Include mapping: `[Spoke Name]: [Which hub sections relate]`
   - Example: `[Query Optimization]: Maps to sections 2.3 (Performance) and 3.1 (Indexing)`

2. **Verify back-reference**:
   - Spoke MUST include hub reference in Context section
   - Use grep: `grep -r "Hub Document:" path/to/spoke.md`
   - If missing: Add back-reference before proceeding

3. **Update Core Concepts**:
   - Review each hub subsection for relevance to new spoke
   - Add cross-reference where strategic relationship exists
   - Do NOT add spoke to every section (only where semantically relevant)

4. **Navigation verification**:
   - Manually test: Can you reach spoke's key implementation details in ≤3 clicks from hub?
   - If >3 clicks: Restructure hub subsections OR add spoke subsection

**Validation**: Hub PR must include evidence of all 4 steps completion.

### Updating a Spoke

When a spoke document is modified:

1. Review if changes affect hub's strategic overview
2. Update hub if canonical definitions changed
3. Verify cross-references remain accurate

### Removing a Spoke

When a spoke is deprecated or merged:

1. Remove from hub's "Related Documents" section
2. Update any core concept sections referencing the spoke
3. Verify no broken cross-references remain

## Validation Checklist

**PRE-MERGE VALIDATION** (All CRITICAL items MUST pass)

**CRITICAL** (PR blocked if failing):

- [ ] Zero structural violations (title, context, decision, core concepts present)
- [ ] Zero orphaned spokes (all spokes linked from hub)
- [ ] Back-reference compliance ≥90%
- [ ] 3-click navigation metric satisfied for all spokes
- [ ] No forbidden anti-patterns (link farm, copy-paste, spoke replacement)

**REQUIRED** (PR requires explicit justification if failing):

- [ ] 3-7 core concept subsections (not <3 or >7)
- [ ] Each subsection includes cross-references
- [ ] Consolidation provides narrative value beyond inventory
- [ ] Bidirectional linking complete (hub↔spokes)

**RECOMMENDED** (does not block PR but should be addressed):

- [ ] Concrete examples included for abstract concepts
- [ ] Diagrams included where relationships complex
- [ ] Hub length <1000 lines

## Related Documents

- **Templates**: See `doc/_meta/templates/hub.md` for canonical hub template with inline guidance
- **Governance**: See `doc/_meta/governance/hub-consolidation.md` for maintenance workflows and RACI matrix
