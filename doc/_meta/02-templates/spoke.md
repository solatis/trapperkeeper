---
doc_type: template
template_for: spoke
status: active
date_created: 2025-11-06
date_updated: 2025-11-07
primary_category: documentation
tags:
  - templates
  - spoke
maintainer: Documentation Team
validation:
  schema_version: 1

  frontmatter:
    required_fields:
      - doc_type
      - status
      - primary_category
      - hub_document
    field_constraints:
      doc_type:
        enum: ["spoke"]
      status:
        enum: ["draft", "active", "deprecated", "superseded"]
      hub_document:
        pattern: ".*\\.md$"
    conditional_constraints:
      - if_field: status
        equals: superseded
        then_required: ["superseded_by"]

  # Spoke documents have flexible section requirements
  # (minimal structure validation, content-driven)
---

# [Spoke Document Title]

<!--
TEMPLATE GUIDANCE: This is the canonical spoke document template.
Replace all [bracketed placeholders] with actual content.
Remove or keep guidance comments based on your preference.
See doc/_meta/standards/hub-and-spoke-architecture.md for complete requirements.

IMPORTANT: NO revision logs, version history, or change tracking sections.
Git is the source of truth for document history.

SPOKE NAMING: Use descriptive filenames that reflect the focused topic
(e.g., tls-certificate-management.md, field-path-resolution.md).
-->

---

doc_type: spoke
status: draft
date_created: YYYY-MM-DD
primary_category: [architecture|api|database|security|performance|validation|configuration|testing|deployment|error-handling]
hub_document: doc/[domain]/README.md
tags:

- [tag1]
- [tag2]

---

<!--
FRONTMATTER GUIDANCE:
- doc_type: Must be "spoke"
- status: Start with "draft", change to "active" when complete
- primary_category: Choose one that best fits this spoke's domain
- hub_document: REQUIRED - Path to parent hub (all hubs are README.md)
- tags: Optional additional classification tags for discovery
NOTE: Document dates are tracked via git history, not frontmatter.

COMPLEMENTARY PATTERNS:
The hub_document frontmatter field provides MACHINE-READABLE metadata for
validation tooling and automated spoke discovery. You MUST also include the
human-readable **Hub Document**: markdown pattern in the Context section below.

These patterns serve different audiences:
- Frontmatter hub_document: For automated validation and tooling
- Markdown **Hub Document**: For human readers navigating the docs

Both are REQUIRED. See doc/_meta/standards/frontmatter-reference.md for details.
-->

## Context

<!--
GUIDANCE: Explain this spoke's focused scope in 1-2 paragraphs.
- What specific problem does this spoke address?
- Why is focused detail needed separate from hub?
- What implementation concerns does this document?

QUALITY CRITERIA:
✓ Clearly defines focused scope
✓ Distinguishes from hub's strategic overview
✓ Explains why detail level is needed
✓ Includes hub back-reference pattern (REQUIRED)
-->

[1-2 paragraphs explaining this spoke's focused scope and why detailed documentation is needed]

**Hub Document**: This document is part of the [Hub Name] architecture. See [Hub Document Link] for strategic overview and relationships to other components.

<!--
HUB BACK-REFERENCE PATTERN:
The **Hub Document**: line above is REQUIRED for all spoke documents.
This provides human-readable context orienting readers to the strategic overview.

Pattern requirements:
- Appears in Context section
- Uses "Hub Document" as the label (bold)
- Links to specific hub document (always README.md)
- Explains relationship (e.g., "part of", "implements", "extends")
- References what hub provides ("strategic overview", "architectural decisions")

This markdown pattern works with the hub_document frontmatter field above.
Both patterns are complementary and REQUIRED.
-->

## [Implementation Section 1]

<!--
GUIDANCE: Break implementation into 3-7 focused sections.
Each section provides specific technical detail the hub delegates.

Section types:
- Technical specifications
- Configuration details
- Code examples
- API contracts
- Error handling patterns
- Performance characteristics
- Security considerations

QUALITY CRITERIA:
✓ Provides tactical implementation detail
✓ Includes concrete examples
✓ Specifies exact behavior and contracts
✓ Cross-references related sections or documents
✓ Avoids duplicating hub strategic content
-->

[Implementation details with concrete examples]

### [Subsection if needed]

[More specific detail]

**Example**:

```[language]
[Concrete code example demonstrating the concept]
```

**Error Handling**: [How errors are handled in this context]

**Cross-References**:

- [Related Spoke or Hub Section]: [What it provides]
- [Related Document]: [How it relates]

## [Implementation Section 2]

[Continue pattern for 3-7 implementation sections total]

## [Implementation Section 3]

[Continue with additional focused sections as needed]

## Edge Cases and Limitations

<!--
GUIDANCE: Document known limitations, edge cases, and constraints.
This section is optional but recommended for complex implementations.

QUALITY CRITERIA:
✓ Identifies known limitations clearly
✓ Explains why limitations exist
✓ Provides workarounds if available
✓ Documents unsupported scenarios explicitly
-->

**Known Limitations**:

- [Limitation 1]: [Why it exists and any workarounds]
- [Limitation 2]: [Why it exists and any workarounds]

**Edge Cases**:

- [Edge case 1]: [Expected behavior]
- [Edge case 2]: [Expected behavior]

## Related Documents

<!--
GUIDANCE: Document relationships to other documents.
- Hub: Already referenced in Context section (don't repeat)
- Dependencies: Documents that must be understood first
- Related Spokes: Sibling documents in same hub
- Extended by: Documents that build on this one

QUALITY CRITERIA:
✓ Enables bidirectional navigation
✓ Distinguishes relationship types
✓ Uses descriptive link text
✓ Explains why relationship matters
-->

**Dependencies** (read these first):

- [Document Name]: [Why it's foundational to understanding this spoke]

**Related Spokes** (siblings in this hub):

- [Spoke Name]: [How it relates - complements, contrasts, prerequisites]

**Extended by** (documents building on this):

- [Document Name]: [What extension it provides]

## Appendix (Optional)

<!--
GUIDANCE: Use appendices for supporting material not essential to main implementation:
- Extended examples
- Reference tables
- Migration guidance from legacy approaches
- Historical context or design rationale

Only include if adds significant value. Most spokes don't need appendices.
-->

### Appendix A: [Topic]

[Supporting content]

---

<!--
VALIDATION CHECKLIST - Remove before finalizing

Spoke Quality Verification:

Structural Requirements:
[ ] Document starts with title (no revision log)
[ ] Frontmatter includes all required fields
[ ] Frontmatter doc_type is "spoke"
[ ] Frontmatter hub_document field is present and points to hub README.md
[ ] Context section exists and explains focused scope
[ ] Context section includes **Hub Document**: back-reference pattern
[ ] Contains 3-7 implementation-focused sections
[ ] Related Documents section shows relationships

Back-Reference Requirements (BOTH patterns required):
[ ] Frontmatter hub_document field present (machine-readable)
[ ] **Hub Document**: markdown pattern in Context (human-readable)
[ ] Both patterns reference the same hub
[ ] Markdown pattern includes link to hub
[ ] Markdown pattern explains hub's strategic role

Content Requirements:
[ ] Provides implementation detail hub delegates
[ ] Includes concrete examples and code samples
[ ] Specifies exact behavior and contracts
[ ] Documents error handling patterns
[ ] Identifies edge cases and limitations
[ ] Uses consistent domain language with hub
[ ] Avoids duplicating hub strategic content

Navigation Requirements:
[ ] Cross-references use descriptive text
[ ] Links to hub work bidirectionally (hub also links here)
[ ] Related Documents shows relationship types
[ ] Maximum 3 clicks from hub to this spoke

Reference Implementation Patterns:
[ ] Hub back-reference in Context (markdown and frontmatter)
[ ] Section-specific detail with concrete examples
[ ] Error handling and edge cases documented
[ ] Clear relationship distinctions in Related Documents

See doc/_meta/standards/hub-and-spoke-architecture.md and
doc/_meta/standards/frontmatter-reference.md for complete requirements.
-->
