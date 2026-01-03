---
doc_type: spoke
status: active
date_created: 2025-11-06
date_updated: 2025-11-07
primary_category: documentation
hub_document: doc/_meta/01-standards/README.md
tags:
  - standards
  - frontmatter
  - metadata
maintainer: Documentation Team
---

# Frontmatter Metadata Reference

## Purpose

This document provides the canonical specification for YAML frontmatter metadata used in Trapperkeeper **product documentation**. It defines all fields, their types, required status, and allowed values for both hub and spoke documents.

## Scope

**This standard applies ONLY to product documentation.**

- ✅ **Applies to**: Any .md file in `doc/` that is NOT in `doc/_meta/`
- ❌ **Does NOT apply to**: Any .md file in `doc/_meta/` (meta-documentation is exempt)

Meta-documentation may optionally use frontmatter when beneficial, but is not required to.

**IMPORTANT: NO revision logs, version history, or change tracking sections in documents. Git is the source of truth for document history. NO version/revision fields in frontmatter.**

This means:

1. **NO version history SECTIONS in document body** - No "Version History", "Revision Log", "Change Log", or similar sections
2. **NO version/revision FIELDS in YAML frontmatter** - No fields named `version`, `revision`, `changelog`, `history`, or similar

Both prohibitions apply to product documentation only (not `doc/_meta/`). Document dates are tracked via git history rather than frontmatter fields. Use `git log --follow <file>` to retrieve date information.

## Schema Files

Formal JSON Schema definitions are maintained in:

- `frontmatter-spoke.schema.json` - Schema for spoke documents
- `frontmatter-hub.schema.json` - Schema for hub documents

## Common Fields (All Documents)

**Remember: No version or revision fields allowed. Git provides full history.**

### doc_type (required)

**Type**: string (enum)
**Allowed Values**: `spoke`, `hub`, `index`, `guide`, `reference`, `redirect-stub`
**Description**: Classifies the document type for tooling and navigation

**Usage**:

- `spoke`: Focused implementation document within a domain
- `hub`: Consolidating document for 3+ related spokes
- `index`: Cross-cutting concern index
- `guide`: Tutorial or how-to document
- `reference`: API or technical reference material
- `redirect-stub`: Redirect for moved or consolidated documents

**Example**:

```yaml
doc_type: spoke
```

### status (required)

**Type**: string (enum)
**Allowed Values**: `draft`, `active`, `deprecated`, `superseded`
**Description**: Current lifecycle status of the document

**Usage**:

- `draft`: Work in progress, not yet authoritative
- `active`: Authoritative and maintained
- `deprecated`: Still valid but discouraged, replacement exists
- `superseded`: Replaced by another document (requires `superseded_by` field)

**Example**:

```yaml
status: active
```

### date_created (deprecated)

**Status**: No longer required. Document dates are tracked via git history.

**Type**: string (date format YYYY-MM-DD)
**Description**: Legacy field for document creation date. Existing fields may remain but are not required for new documents.

### date_updated (deprecated)

**Status**: No longer required. Document dates are tracked via git history.

**Type**: string (date format YYYY-MM-DD)
**Description**: Legacy field for update tracking. Use `git log --follow <file>` instead.

### primary_category (required)

**Type**: string (enum)
**Allowed Values**: `architecture`, `api`, `database`, `security`, `performance`, `validation`, `configuration`, `testing`, `deployment`, `error-handling`
**Description**: Primary domain category for classification and navigation

**Example**:

```yaml
primary_category: security
```

### tags (optional)

**Type**: array of strings
**Description**: Additional classification tags for discovery and filtering. Free-form but prefer domain-specific terms.

**Example**:

```yaml
tags:
  - authentication
  - tls
  - encryption
  - compliance
```

### authors (optional)

**Type**: array of strings
**Description**: Primary authors or contributors to the document

**Example**:

```yaml
authors:
  - Alice Engineer
  - Bob Architect
```

## Hub-Specific Fields

### consolidated_spokes (required for hubs)

**Type**: array of strings
**Minimum Items**: 3
**Description**: List of spoke document paths or identifiers consolidated by this hub. Minimum 3 spokes required.

**Example**:

```yaml
consolidated_spokes:
  - doc/security/authentication.md
  - doc/security/authorization.md
  - doc/security/tls-configuration.md
```

### maintainer (optional but recommended for hubs)

**Type**: string
**Description**: Primary maintainer responsible for hub content accuracy and spoke synchronization

**Example**:

```yaml
maintainer: Security Team
```

## Index-Specific Fields

**Related Documents**:

- **Governance**: `doc/_meta/governance/cross-cutting-index-governance.md` - Index maintenance procedures and ownership model
- **Template**: `doc/_meta/templates/cross-cutting-index.md` - Canonical structure with inline guidance

### maintainer (required for indexes)

**Type**: string
**Description**: Primary maintainer responsible for index accuracy and quarterly review

**Example**:

```yaml
maintainer: Security Team
```

### last_review (required for indexes)

**Type**: string (date format YYYY-MM-DD)
**Description**: Date of last quarterly review

**Example**:

```yaml
last_review: 2025-11-01
```

### next_review (required for indexes)

**Type**: string (date format YYYY-MM-DD)
**Description**: Date of next scheduled quarterly review

**Example**:

```yaml
next_review: 2026-02-01
```

## Spoke-Specific Fields

### hub_document (required for spokes)

**Type**: string
**Description**: Machine-readable reference to the hub document this spoke belongs to. Use relative path or document identifier. This frontmatter field provides metadata for validation tooling and automated spoke discovery.

**Relationship to Markdown Pattern**: Spoke documents MUST include BOTH this frontmatter field and the human-readable markdown pattern `**Hub Document**:` in the Context section (see `hub-and-spoke-architecture.md`). These patterns are complementary:

- **Frontmatter `hub_document`**: Machine-readable for validation tooling, spoke discovery, and automated relationship verification
- **Markdown `**Hub Document**:`**: Human-readable prose explanation in Context section orienting readers to strategic overview

Both patterns serve different audiences and purposes. Do not treat them as alternatives.

**Hub Filename Convention**: All hub documents MUST be named `README.md` per the naming convention in `hub-and-spoke-architecture.md`. This ensures platform rendering, directory uniqueness, and consistent discoverability.

**Example**:

```yaml
hub_document: doc/security/README.md
```

## Relationship Fields (All Documents)

### depends_on (optional)

**Type**: array of strings
**Description**: Documents that must be understood before this one. Use document identifiers or relative paths. Establishes prerequisite reading order.

**Example**:

```yaml
depends_on:
  - doc/architecture/core-principles.md
  - doc/api/protocol-design.md
```

### extended_by (optional)

**Type**: array of strings
**Description**: Documents that extend or build upon this one. Creates forward references to related content.

**Example**:

```yaml
extended_by:
  - doc/security/advanced-tls.md
  - doc/security/certificate-rotation.md
```

### superseded_by (required if status is 'superseded')

**Type**: string
**Description**: Document identifier that supersedes this one. Required when status is 'superseded'.

**Example**:

```yaml
status: superseded
superseded_by: doc/security/unified-authentication.md
```

## Cross-Cutting Concern Fields

### cross_cutting (optional)

**Type**: array of strings (enum)
**Allowed Values**: `security`, `performance`, `validation`, `observability`, `error-handling`
**Description**: Cross-cutting concerns this document addresses. Used for generating cross-cutting indexes.

**Example**:

```yaml
cross_cutting:
  - security
  - performance
```

## Complete Examples

### Example: Hub Document Frontmatter

```yaml
---
doc_type: hub
status: active
date_created: 2025-11-04
date_updated: 2025-11-05
primary_category: security
tags:
  - authentication
  - encryption
  - compliance
consolidated_spokes:
  - doc/security/authentication-web-ui.md
  - doc/security/authentication-sensor-api.md
  - doc/security/tls-configuration.md
cross_cutting:
  - security
depends_on:
  - doc/architecture/core-principles.md
maintainer: Security Team
authors:
  - Alice Engineer
  - Bob Architect
---
```

### Example: Spoke Document Frontmatter

```yaml
---
doc_type: spoke
status: active
date_created: 2025-11-05
primary_category: security
tags:
  - authentication
  - sessions
  - cookies
hub_document: doc/security/README.md
cross_cutting:
  - security
depends_on:
  - doc/security/README.md
authors:
  - Alice Engineer
---
```

### Example: Superseded Document Frontmatter

```yaml
---
doc_type: spoke
status: superseded
date_created: 2025-10-28
date_updated: 2025-11-05
primary_category: validation
superseded_by: doc/validation/unified-validation.md
tags:
  - deprecated
---
```

## Field Naming Convention

This project uses present tense for field names to maintain consistency across all relationship fields (`depends_on`, `extended_by`, `consolidated_spokes`).

## Validation

Frontmatter validation is performed by:

1. JSON Schema validation against schema files
2. Automated CI/CD checks on all pull requests
3. Manual review for semantic correctness

Run validation locally:

```bash
python doc/scripts/validate-frontmatter.py --strict
```

## Cross-Reference with Schemas

This reference document provides human-readable documentation. For programmatic validation, always refer to the JSON Schema files:

- `frontmatter-spoke.schema.json` - Authoritative schema for spokes
- `frontmatter-hub.schema.json` - Authoritative schema for hubs

## Version History

| Date       | Description                                |
| ---------- | ------------------------------------------ |
| 2025-11-06 | Initial version with hub and spoke schemas |
