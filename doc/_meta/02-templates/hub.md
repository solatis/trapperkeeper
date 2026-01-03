---
doc_type: template
template_for: hub
status: active
date_updated: 2025-11-07
primary_category: documentation
tags:
  - templates
  - hub
maintainer: Documentation Team
validation:
  schema_version: 1

  # Hub documents must be named README.md
  filename_pattern: "^README\\.md$"

  frontmatter:
    required_fields:
      - doc_type
      - status
      - primary_category
      - consolidated_spokes
    field_constraints:
      doc_type:
        enum: ["hub"]
      status:
        enum: ["draft", "active", "deprecated", "superseded"]
      consolidated_spokes:
        type: array
        min_items: 3
    conditional_constraints:
      - if_field: status
        equals: superseded
        then_required: ["superseded_by"]

  required_sections:
    - name: "Context"
      must_exist: true
      min_paragraphs: 2
      max_paragraphs: 4

    - name: "Decision"
      must_exist: true
      subsections_required:
        min: 3
        max: 7
        pattern: "^### "

    - name: "Consequences"
      must_exist: true

    - name: "Related Documents"
      must_exist: true
---

# [Domain Name] Architecture

<!--
TEMPLATE GUIDANCE: This is the canonical hub document template.
Replace all [bracketed placeholders] with actual content.
Remove or keep guidance comments based on your preference.
See doc/_meta/standards/hub-and-spoke-architecture.md for complete requirements.

IMPORTANT: NO revision logs, version history, or change tracking sections.
Git is the source of truth for document history.

HUB FILENAME CONVENTION: All hub documents MUST be saved as README.md
in their respective domain directory (e.g., doc/security/README.md).
This convention ensures platform rendering, directory uniqueness, and
consistent discoverability across the documentation.

FRONTMATTER REQUIREMENTS:
- doc_type: MUST be 'hub'
- status: active|draft|deprecated|superseded
- primary_category: Choose ONE from list above
- consolidated_spokes: List of spoke files (minimum 3 for product docs, can be fewer for _meta/)
- tags: Relevant keywords for discovery
- cross_cutting: List of cross-cutting concerns this hub addresses (optional)
- maintainer: Team or person responsible for this hub
NOTE: Document dates are tracked via git history, not frontmatter.
-->

## Context

<!--
REQUIRED SECTION: Must explain why this hub exists.

GUIDANCE: Write 2-4 paragraphs addressing:
1. What fragmentation currently exists across documents?
2. What pain points does this fragmentation create?
3. Why is consolidation necessary?
4. What problem does a unified strategy solve?

QUALITY CRITERIA:
✓ Explains the fragmentation problem clearly
✓ Articulates pain points of current state
✓ Justifies need for consolidation
✓ Avoids solution details (save for Decision section)

LENGTH: 2-4 paragraphs (aim for ~200-400 words)
-->

[Paragraph 1: Current fragmentation]

[Paragraph 2: Pain points created by fragmentation]

[Paragraph 3: Why consolidation is necessary]

[Paragraph 4 (optional): What unified strategy solves]

## Decision

<!--
REQUIRED SECTION: State the unified strategic decision.

GUIDANCE:
1. Open with clear decision statement: "We will implement **[strategy]** with [characteristics]"
2. Explicitly state: "This document serves as the hub..."
3. Explain core principles and constraints
4. Show how this addresses the Context section
5. Do NOT include implementation details (delegate to spokes)

QUALITY CRITERIA:
✓ Opens with clear decision statement
✓ Explicitly identifies as hub document
✓ Explains strategic approach
✓ References spoke documents for details
✓ Avoids implementation minutiae

LENGTH: 1-2 paragraphs intro + 3-7 concept area subsections
-->

We will implement **[unified strategy name]** with [key characteristics: principles, scope, constraints].

This document serves as the [domain name] hub providing strategic overview with cross-references to detailed implementation documents. It consolidates [list the 3+ spokes being consolidated] into a cohesive strategy addressing [the fragmentation described in Context].

### [Major Concept Area 1]

<!--
REQUIRED: 3-7 concept area subsections (one per major concern in this domain).

MANDATORY STRUCTURE per subsection:
1. Strategic overview: 2-3 paragraphs
2. Key Points: 3-5 bullets
3. Cross-References: Links to spoke sections
4. Example: 1-2 sentences with concrete illustration

QUALITY CRITERIA:
✓ Strategic not tactical focus
✓ Establishes canonical definitions
✓ Cross-references spokes for details
✓ Includes concrete example
✓ Avoids duplicating spoke content

LENGTH: ~150-300 words per subsection
-->

[Paragraph 1: Define this concept area and why it matters]

[Paragraph 2: Strategic approach to this concern]

[Paragraph 3 (optional): How it relates to other concept areas]

**Key Points:**

- [Essential point 1: What this concept ensures/provides]
- [Essential point 2: Core principle or constraint]
- [Essential point 3: Strategic trade-off or decision]
- [Essential point 4 (optional)]
- [Essential point 5 (optional)]

**Cross-References:**

- **[Spoke Document Name]** Section [N]: [What specific detail it provides]
- **[Spoke Document Name]** Section [N]: [What specific detail it provides]
- **[Another Hub]**: [How it relates or constrains this concept]

**Example**: [1-2 sentences with concrete code, configuration, or scenario illustrating this concept]

### [Major Concept Area 2]

[Repeat pattern above for each major concept area]

### [Major Concept Area 3]

[Continue for 3-7 concept areas total]

## Consequences

<!--
GUIDANCE: Explain benefits and trade-offs of this approach.
- What improves with unified strategy?
- What are the trade-offs?
- How does this impact maintenance?
- Are there risks or limitations?

QUALITY CRITERIA:
✓ Honest assessment of benefits
✓ Acknowledges trade-offs
✓ Considers maintenance implications
✓ Identifies risks if applicable
-->

**Benefits:**

- [Specific improvement 1]
- [Specific improvement 2]
- [Specific improvement 3]

**Trade-offs:**

- [What we're giving up or accepting]
- [Complexity added or shifted]

## Related Documents

<!--
GUIDANCE: Document all relationships to other documents.
- Consolidated Spokes: documents this hub consolidates (with section mappings)
- Dependencies: foundational documents this hub builds upon
- References: related hubs or cross-cutting documents
- Extended by: future documents that may extend this hub

QUALITY CRITERIA:
✓ Lists all consolidated spokes
✓ Includes section mappings for spokes
✓ Documents dependencies clearly
✓ Distinguishes relationship types
✓ Enables bidirectional navigation
-->

**Consolidated Spokes** (this hub consolidates):

- [Spoke Document Name] Section [N]: Maps to this hub's Section [M]
- [Spoke Document Name] Section [N]: Maps to this hub's Section [M]
- [Spoke Document Name]: Overall guidance provided in Section [M]

**Dependencies** (foundational documents):

- [Document Name]: [Why this is foundational to this hub]

**References** (related hubs/documents):

- [Document Name]: [How it relates - e.g., "complements", "constrains", "implements"]

**Extended by**:

- [Document Name]: [What specific extension it provides]

## Appendix (Optional)

<!--
GUIDANCE: Use appendices for supporting material not essential to main narrative:
- Extended examples
- Reference tables
- Migration guidance
- Historical context

Only include if adds significant value. Many hubs don't need appendices.
-->

### Appendix A: [Topic]

[Supporting content]

---

<!--
VALIDATION CHECKLIST - Remove before finalizing

Hub Quality Verification:

Structural Requirements:
[ ] Document starts with title (no revision log)
[ ] Context section exists and explains fragmentation problem
[ ] Decision section explicitly states this is a hub document
[ ] Contains 3-7 core concept subsections
[ ] Related Documents section lists all consolidated spokes
[ ] Cross-references use descriptive text, not document numbers alone

Navigational Requirements:
[ ] Maximum 3 clicks from hub to any spoke detail
[ ] Minimum 90% of spokes back-reference this hub
[ ] Bidirectional linking: hub→spokes and spokes→hub
[ ] Each subsection includes explicit cross-references
[ ] No orphaned spokes (unlinked from hub)

Content Requirements:
[ ] Provides narrative context beyond spoke inventory
[ ] Consolidates rather than duplicates spoke content
[ ] Explains relationships between concepts
[ ] Uses domain language consistently
[ ] Includes concrete examples for abstract concepts
[ ] Avoids implementation details (delegate to spokes)

Consolidation Semantics:
[ ] Hub abstracts core concerns while spokes provide implementation detail
[ ] Hub provides narrative value beyond mere spoke inventory
[ ] Hub establishes canonical definitions used by spokes
[ ] Hub explains relationships and trade-offs between approaches

Reference Implementation Patterns:
[ ] Explicit hub designation in Decision section
[ ] Section-specific mappings in Related Documents
[ ] Layered presentation (strategic to tactical)
[ ] 2-3 cross-references per major subsection

See doc/_meta/standards/hub-and-spoke-architecture.md for complete requirements.
-->
