---
doc_type: hub
status: active
date_created: 2025-11-07
primary_category: documentation
consolidated_spokes:
  - architecture.md
  - linters.md
tags:
  - meta-documentation
  - tooling
  - validation
  - automation
maintainer: Documentation Team
---

# Documentation Validation Tooling

## Context

Documentation standards and governance procedures require automated validation to ensure compliance. Manual validation is error-prone, inconsistent, and doesn't scale. Without validation tooling, documentation quality drifts over time as standards evolve and new documents are created.

Validation must be fast, comprehensive, and CI/CD integrated. Python stdlib-only implementation avoids external dependencies simplifying deployment. Validation errors need clear messaging guiding authors to fix issues.

## Decision

We will implement **comprehensive validation tooling** automating documentation quality checks with CI/CD integration.

This document serves as the tooling hub documenting validation architecture, algorithms, and integration procedures. Validation covers frontmatter schema, hub-spoke relationships, CLAUDE.md format, cross-cutting indexes, and link integrity.

### Validation Architecture

Validation architecture defines subcommands, algorithms, error formats, and execution flow.

**Key Points:**

- Python stdlib-only implementation (no external dependencies)
- Subcommands: validate-frontmatter, validate-hub-spoke, validate-claude-md, validate-indexes, validate-links, validate-all
- Clear error messages with file paths and fix guidance
- Zero exit code on success, non-zero on validation failures
- CI/CD integration blocks merges on validation errors

**Cross-References:**

- **architecture.md**: Complete validation architecture, pseudocode algorithms, error formats, subcommand specifications, CI/CD integration

**Example**: `python doc/scripts/validate.py validate-all` runs all validators sequentially, returning first non-zero exit code on failure.

### Validation Algorithms

Validation algorithms implement specific checks for each documentation standard.

**Key Points:**

- Frontmatter validation: schema compliance, required fields, enum values, date formats
- Hub-spoke validation: bidirectional relationships, 100% back-reference compliance, orphan detection
- CLAUDE.md validation: format compliance, trigger patterns, length limits, forbidden content
- Index validation: completeness, link integrity (placeholder)
- Link validation: internal link integrity, broken link detection (placeholder)

**Cross-References:**

- **architecture.md** Sections 2-6: Detailed algorithms with pseudocode for each validation type

**Example**: Hub-spoke validator resolves relative spoke references, checks existence, verifies hub_document back-reference fields match hub path.

### CI/CD Integration

CI/CD integration ensures validation runs on every documentation change with merge gates blocking invalid submissions.

**Key Points:**

- Pre-commit hooks run validation locally
- GitHub Actions / CI pipeline runs validation on pull requests
- Merge blocked if validation fails (non-zero exit)
- Clear error reporting in CI/CD logs guides fix actions

**Cross-References:**

- **architecture.md** Section 7: CI/CD integration procedures, hook installation, pipeline configuration

**Example**: GitHub Actions job runs `validate-all` on doc/ changes, blocking PR merge if frontmatter missing required fields.

## Consequences

**Benefits:**

- Automated validation enforces standards uniformly
- Fast feedback loop catches errors early
- CI/CD integration prevents invalid documentation merges
- Clear error messages guide authors to fixes
- Python stdlib-only simplifies deployment and maintenance

**Trade-offs:**

- Validation adds CI/CD execution time for documentation changes
- Requires maintaining validation scripts as standards evolve
- May feel constraining when validation blocks legitimate edge cases

## Related Documents

**Consolidated Spokes** (this hub consolidates):

- **architecture.md**: Maps to Validation Architecture and Validation Algorithms subsections
- **linters.md**: Additional linting tools and editor integrations

**Dependencies** (foundational documents):

- **doc/\_meta/01-standards/README.md**: Standards that validation enforces
- **doc/\_meta/README.md**: Parent meta-documentation hub

**References** (related hubs):

- **doc/\_meta/03-governance/README.md**: Governance procedures enforced through validation
