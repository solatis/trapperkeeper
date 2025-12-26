---
doc_type: template
template_for: cross-cutting-index
status: active
date_created: 2025-11-06
date_updated: 2025-11-08
primary_category: documentation
hub_document: README.md
tags:
  - templates
  - index
  - cross-cutting
maintainer: Documentation Team
validation:
  schema_version: 1

  frontmatter:
    required_fields:
      - doc_type
      - status
      - date_created
      - primary_category
      - cross_cutting_concern
    field_constraints:
      doc_type:
        enum: ["index"]
      status:
        enum: ["draft", "active", "deprecated", "superseded"]
      date_created:
        pattern: "^\\d{4}-\\d{2}-\\d{2}$"
      cross_cutting_concern:
        enum: ["security", "performance", "validation", "observability", "error-handling"]
    conditional_constraints:
      - if_field: status
        equals: superseded
        then_required: ["superseded_by"]

  required_sections:
    - name: "Overview"
      must_exist: true
      min_paragraphs: 1
      max_paragraphs: 3
    - name: "Consolidated Documents"
      must_exist: true
---

# [Cross-Cutting Concern] Index

<!--
TEMPLATE GUIDANCE: This is the canonical cross-cutting index template.
Cross-cutting indexes provide navigation entry points for concerns that span multiple domains.

Standard cross-cutting concerns:
- Security
- Performance
- Validation
- Observability
- Error Handling

Replace all [bracketed placeholders] with actual content.

REQUIRED READING BEFORE USING THIS TEMPLATE:
- Governance: doc/_meta/governance/cross-cutting-index-governance.md - Maintenance procedures and ownership model
- Frontmatter Schema: doc/_meta/standards/frontmatter-reference.md - Required metadata fields for indexes
-->

---

doc_type: index
status: active
date_created: YYYY-MM-DD
date_updated: YYYY-MM-DD
primary_category: [category]
cross_cutting:

- [concern-name]
  maintainer: [Team or Individual]
  last_review: YYYY-MM-DD
  next_review: YYYY-MM-DD

---

## Purpose

<!--
GUIDANCE: Explain in 2-3 sentences:
- What cross-cutting concern this index covers
- Who should use this index
- How this index helps navigation
-->

This index provides navigation to all documentation addressing [cross-cutting concern] across the Trapperkeeper system. Use this as a discovery mechanism for [concern]-related decisions, patterns, and implementations regardless of their primary domain.

## Quick Reference

<!--
GUIDANCE: Provide 3-5 high-level categories organizing the index content.
Each category should represent a major aspect of the cross-cutting concern.
Link to hub documents where available, specific sections otherwise.
-->

| Category     | Description              | Key Documents          |
| ------------ | ------------------------ | ---------------------- |
| [Category 1] | [1-sentence description] | [Hub Name], [Doc Name] |
| [Category 2] | [1-sentence description] | [Hub Name], [Doc Name] |
| [Category 3] | [1-sentence description] | [Hub Name], [Doc Name] |

## Core Concepts

<!--
GUIDANCE: For each major aspect of the cross-cutting concern:
- Use subsection (###) headings
- Provide 2-3 sentence overview
- List relevant hub and spoke documents
- Include section-specific links for precision

Aim for 3-7 subsections total.
-->

### [Major Aspect 1]

[2-3 sentences explaining this aspect of the cross-cutting concern]

**Relevant Documentation:**

- **[Hub Document Name]** - [What it covers] → See Section [N] for [specific topic]
- **[Spoke Document Name]** - [What it covers] → See Section [N] for [specific topic]
- **[Spoke Document Name]** - [What it covers]

### [Major Aspect 2]

[2-3 sentences explaining this aspect of the cross-cutting concern]

**Relevant Documentation:**

- **[Hub Document Name]** - [What it covers]
- **[Spoke Document Name]** - [What it covers]

### [Major Aspect 3]

[Continue pattern for 3-7 subsections]

## Domain Coverage Matrix

<!--
GUIDANCE: Show which domains address this cross-cutting concern.
Helps readers understand breadth of coverage and identify gaps.
Use checkmarks for domains that address the concern.
-->

| Domain         | Coverage | Key Document    |
| -------------- | -------- | --------------- |
| Architecture   | ✓        | [Document Name] |
| API Design     | ✓        | [Document Name] |
| Database       | ✓        | [Document Name] |
| Security       | ✓        | [Document Name] |
| Performance    | ✓        | [Document Name] |
| Validation     | ✓        | [Document Name] |
| Configuration  | ✓        | [Document Name] |
| Testing        | ✓        | [Document Name] |
| Deployment     | ✓        | [Document Name] |
| Error Handling | ✓        | [Document Name] |

## Patterns and Best Practices

<!--
GUIDANCE: Highlight recurring patterns for this cross-cutting concern.
Each pattern should:
- Have a descriptive name
- Brief description (1-2 sentences)
- Reference to detailed documentation
-->

### [Pattern Name 1]

**Description**: [1-2 sentences describing the pattern]

**Used In**:

- [Document Name] Section [N]
- [Document Name] Section [N]

### [Pattern Name 2]

**Description**: [1-2 sentences describing the pattern]

**Used In**:

- [Document Name] Section [N]

## Related Indexes

<!--
GUIDANCE: Link to other cross-cutting indexes that have overlap.
Explain the relationship between concerns.
-->

- **[Related Index Name]**: [How it relates to this concern]
- **[Related Index Name]**: [How it relates to this concern]

## Maintenance Notes

<!--
GUIDANCE: Document when index was last reviewed and next review date.
Note any gaps or areas needing expansion.
This section helps maintainers track index health.
-->

**Last Updated**: YYYY-MM-DD
**Last Review**: YYYY-MM-DD
**Next Review**: YYYY-MM-DD (quarterly)
**Maintainer**: [Team or Individual]

**Known Gaps**:

- [Area needing documentation]
- [Area needing documentation]

**Planned Additions**:

- [Upcoming document or section]

---

<!--
VALIDATION CHECKLIST - Remove before finalizing

Index Quality Verification:

Structure:
[ ] Frontmatter includes all required fields
[ ] Purpose section explains index scope clearly
[ ] Quick Reference table with 3-5 categories
[ ] Core Concepts with 3-7 subsections
[ ] Domain Coverage Matrix shows breadth
[ ] Maintenance Notes section present

Content:
[ ] All links use descriptive text
[ ] Section-specific references where helpful
[ ] Patterns identified and documented
[ ] Related indexes cross-referenced
[ ] No duplicate entries

Maintenance:
[ ] Maintainer assigned in frontmatter
[ ] Last review date current (within 3 months)
[ ] Next review date set (quarterly)
[ ] Known gaps documented honestly

Navigation:
[ ] Links to hub documents where available
[ ] Links to spoke sections when specific detail needed
[ ] Maximum 3 clicks to reach detailed content
[ ] Bidirectional: documents link back to index via cross_cutting frontmatter

See doc/_meta/governance/cross-cutting-index-governance.md for complete requirements.
-->
