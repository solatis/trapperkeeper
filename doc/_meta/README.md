---
doc_type: hub
status: active
date_updated: 2025-11-07
primary_category: documentation
consolidated_spokes:
  - 01-standards/hub-and-spoke-architecture.md
  - 01-standards/frontmatter-reference.md
  - 01-standards/claude-md-format.md
  - 02-templates/hub.md
  - 02-templates/claude-md.md
  - 02-templates/cross-cutting-index.md
  - 03-governance/hub-consolidation.md
  - 03-governance/cross-cutting-index-governance.md
  - 04-tooling/architecture.md
tags:
  - meta-documentation
  - standards
  - templates
  - governance
maintainer: Documentation Team
---

# Documentation Meta-Documentation

## Context

Documentation about how to write documentation was initially treated as exempt from its own rules, creating inconsistency and making meta-documentation harder to discover and validate. This exemption prevented automated tooling from working uniformly across all documentation and created confusion about which patterns apply where.

This hub consolidates all meta-documentation standards, templates, governance procedures, and tooling architecture following the same hub-and-spoke pattern used for product documentation. Meta-documentation now uses frontmatter, hub documents (README.md), and CLAUDE.md navigation files following identical patterns to product docs.

## Decision

Meta-documentation in `_meta/` is **exempt from strict frontmatter requirements** per the policy in `01-standards/frontmatter-reference.md`. Meta-docs may optionally use frontmatter when beneficial but are not required to. This exemption allows tooling docs, templates, and governance documents to focus on content without metadata overhead.

This document serves as the meta-documentation hub providing navigation to standards (how to write docs), templates (copy-paste starting points), governance (procedures and workflows), and tooling (validation automation). Meta-documentation follows the same structural patterns as product documentation while documenting the rules themselves.

**Exception**: Every subdirectory in `_meta/` (01-standards/, 02-templates/, 03-governance/, 04-tooling/) MUST have a hub document (README.md) regardless of spoke count, because meta-documentation benefits from strategic overview even with few spokes.

**Date Tracking**: Document creation and modification dates are tracked via git history rather than frontmatter fields. Use `git log --follow <file>` to retrieve date information. This avoids redundant metadata that drifts out of sync with actual changes.

### Standards (How to Write Documentation)

Standards define requirements, quality criteria, and validation rules for all documentation.

**Key Points:**

- Hub-and-spoke architecture standard defines when and how to create hub documents
- Frontmatter reference specifies required metadata fields for all documentation
- CLAUDE.md format standard defines navigation file structure
- Standards are prescriptive (what MUST be done), not descriptive (what currently exists)

**Cross-References:**

- **01-standards/hub-and-spoke-architecture.md**: Complete hub creation criteria, quality thresholds, navigation requirements
- **01-standards/frontmatter-reference.md**: Field definitions, required vs optional fields, allowed values
- **01-standards/claude-md-format.md**: CLAUDE.md structure, triggers, forbidden content

### Templates (Copy-Paste Starting Points)

Templates provide canonical starting points with inline guidance for creating documentation.

**Key Points:**

- Hub template includes frontmatter, section structure, quality criteria, validation checklist
- CLAUDE.md template enforces fast-index format with opening triggers
- All templates include guidance comments explaining each section
- Delete guidance comments before committing final documentation

**Cross-References:**

- **02-templates/hub.md**: Hub document template with MANDATORY sections and structure markers
- **02-templates/claude-md.md**: CLAUDE.md template with good/bad trigger examples
- **02-templates/cross-cutting-index.md**: Index template for security, performance, validation, observability, error-handling

### Governance (Procedures and Workflows)

Governance documents define procedures, workflows, and responsibilities for maintaining documentation.

**Key Points:**

- Hub consolidation governance defines when to create hubs vs spokes
- Cross-cutting index governance defines ownership and update triggers
- Lightweight governance suited for 5-person team
- Every \_meta/ subdirectory must have a hub regardless of spoke count

**Cross-References:**

- **03-governance/hub-consolidation.md**: Hub creation thresholds, update workflows, quarterly reviews
- **03-governance/cross-cutting-index-governance.md**: Index ownership, conflict resolution, automation strategy

### Tooling (Validation Automation)

Tooling documents define validation automation architecture, algorithms, and CI/CD integration.

**Key Points:**

- validate.py implements all validation checks (frontmatter, hub-spoke, indexes, links, CLAUDE.md, freshness)
- Python stdlib-only implementation (no external dependencies)
- CI/CD integration with merge gates on validation failures
- Validation architecture documented with pseudocode and error formats

**Cross-References:**

- **04-tooling/architecture.md**: Complete validation architecture, subcommands, algorithms, CI/CD integration

## Consequences

**Benefits:**

- Uniform validation applies to all documentation (no special cases for \_meta/)
- Meta-documentation is discoverable through same navigation patterns
- Automated tooling works consistently across all docs
- CLAUDE.md navigation files provide fast access to meta-docs
- Frontmatter enables cross-referencing and automated index generation

**Trade-offs:**

- Meta-documentation must maintain its own frontmatter (small overhead)
- More structured than freeform prose (enforces consistency)
- Must update hub when adding new meta-docs (same workflow as product docs)

## Related Documents

**Consolidated Spokes** (this hub consolidates):

- **01-standards/hub-and-spoke-architecture.md**: Maps to Standards subsection
- **01-standards/frontmatter-reference.md**: Maps to Standards subsection
- **01-standards/claude-md-format.md**: Maps to Standards subsection
- **02-templates/hub.md**: Maps to Templates subsection
- **02-templates/claude-md.md**: Maps to Templates subsection
- **02-templates/cross-cutting-index.md**: Maps to Templates subsection
- **03-governance/hub-consolidation.md**: Maps to Governance subsection
- **03-governance/cross-cutting-index-governance.md**: Maps to Governance subsection
- **04-tooling/architecture.md**: Maps to Tooling subsection

**Dependencies** (foundational documents):

- `../CLAUDE.md`: Root navigation entry point for all documentation

**References** (related hubs):

- Category hubs (01-principles, 02-architecture, etc.): Implement the patterns defined in this hub
