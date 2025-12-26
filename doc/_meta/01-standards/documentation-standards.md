---
doc_type: spoke
status: active
date_created: 2025-11-08
primary_category: documentation
hub_document: README.md
tags:
  - standards
  - validation
  - templates
  - quality
maintainer: Documentation Team
---

# Documentation Standards and Validation Requirements

## Context

All Trapperkeeper documentation must comply with structural, metadata, and content standards to ensure consistency, navigability, and automated validation. These standards apply to all document types: hubs, spokes, indexes, CLAUDE.md files, and templates.

Standards are enforced through template-driven validation rules defined in template frontmatter and executed by `validate.py` during CI checks. This document specifies validation requirements and enforcement mechanisms.

**Hub Document**: This document is part of the Documentation Standards architecture. See [README.md](README.md) for strategic overview of all documentation standards including hub-and-spoke architecture, frontmatter metadata, and CLAUDE.md format.

## Required Validation Frontmatter

All templates must include a `validation` block in frontmatter defining structural and content requirements.

### Schema Version Requirement

**Mandatory field**: Every validation block must declare `schema_version: 1` as first field.

**Purpose**: Ensures forward compatibility when DSL evolves.

**Example**:
```yaml
validation:
  schema_version: 1
  title_pattern: "^# .+ Guide$"
```

**Validation enforcement**: CI checks reject templates without schema_version field.

### Template Validation Coverage

Templates must include validation rules covering:

1. **Title format** (title_pattern): Ensures consistent document naming
2. **Document length** (max_lines): Prevents bloated documents
3. **Content restrictions** (forbidden): Detects anti-patterns
4. **Section structure** (required_sections): Enforces architecture
5. **Frontmatter metadata** (frontmatter): Validates required fields

**Minimal validation**: At minimum, templates must specify `schema_version` and `title_pattern`.

**Comprehensive validation**: Templates for major document types (hub, spoke, CLAUDE.md) must include all five coverage areas.

## Validation Requirements by Document Type

### Hub Documents

**Filename requirement**: All hubs must be named `README.md` (enforced via `filename_pattern: "^README\\.md$"`).

**Frontmatter requirements**:
```yaml
frontmatter:
  required_fields:
    - doc_type
    - status
    - date_created
    - primary_category
    - consolidated_spokes
  field_constraints:
    doc_type:
      enum: ["hub"]
    status:
      enum: ["draft", "active", "deprecated", "superseded"]
    date_created:
      pattern: "^\\d{4}-\\d{2}-\\d{2}$"
    consolidated_spokes:
      type: array
      min_items: 3
  conditional_constraints:
    - if_field: status
      equals: superseded
      then_required: ["superseded_by"]
```

**Section requirements**: Hubs must include Context, Decision (with 3-7 subsections), Consequences, and Related Documents sections.

**Validation template**: See `doc/_meta/02-templates/hub.md` for complete validation block.

### Spoke Documents

**Frontmatter requirements**:
```yaml
frontmatter:
  required_fields:
    - doc_type
    - status
    - date_created
    - primary_category
    - hub_document
  field_constraints:
    doc_type:
      enum: ["spoke"]
    hub_document:
      pattern: ".*\\.md$"
  conditional_constraints:
    - if_field: status
      equals: superseded
      then_required: ["superseded_by"]
```

**Back-reference requirement**: All spokes must include `hub_document` field pointing to parent hub (enforced via conditional_constraints).

**Validation template**: See `doc/_meta/02-templates/spoke.md` for complete validation block.

### CLAUDE.md Navigation Files

**Title requirement**: Must end with "Guide for LLM Agents" (enforced via `title_pattern: "^# .+ Guide for LLM Agents$"`).

**Length requirement**: Maximum 50 lines (60 for root doc/CLAUDE.md).

**Forbidden patterns**:
```yaml
forbidden:
  - pattern: "(?i)how to"
    reason: "how-to instructions belong in implementation docs"
    severity: error
  - pattern: "(?i)step 1"
    reason: "step-by-step procedures violate navigation format"
    severity: error
  - pattern: "(?i)contains information"
    reason: "explanatory content belongs in target documents"
    severity: error
```

**Conditional sections**: Hub, Files, and Subdirectories sections required only if corresponding filesystem entities exist.

**Validation template**: See `doc/_meta/02-templates/claude-md.md` for complete validation block with conditions and files_rules.

### Templates

**Template frontmatter**: Templates must include `template_for` field identifying target document type.

**Validation block requirement**: All templates must include comprehensive validation rules demonstrating correct usage of DSL.

**Schema compliance**: Validation blocks must validate against `validation_schema.json`.

## Validation DSL Syntax

The validation DSL supports eight rule types and four predicates for conditional logic.

### Core Rule Types

1. **title_pattern**: Regex for first H1 heading
2. **max_lines**: Maximum document length
3. **filename_pattern**: Required filename pattern
4. **forbidden**: Prohibited content patterns with severity
5. **required_sections**: Section structure requirements
6. **files_rules**: File listing validation (within sections)
7. **frontmatter**: Metadata field validation
8. **conditions**: Named predicate expressions for conditional rules

### Predicates for Conditional Logic

1. **file_exists(path)**: Filesystem check for specific file
2. **md_files_exist(exclude=[])**: Check for markdown files
3. **subdirs_exist()**: Check for subdirectories
4. **section_present(name)**: Check for section in document AST

### Conditional Operators

- **require_if**: Section required when condition true
- **forbid_if**: Section forbidden when condition true

**Example**:
```yaml
conditions:
  readme_exists: file_exists("README.md")
required_sections:
  - name: "Hub"
    require_if: readme_exists
```

**Complete syntax reference**: See `doc/_meta/04-tooling/validation-dsl-reference.md` for comprehensive DSL documentation.

## CI Validation Enforcement

All documentation changes must pass validation before merging.

### Validation Commands

**Frontmatter validation**:
```bash
./doc/scripts/validate.py frontmatter
```

Checks all documents for required frontmatter fields, enum values, date formats, and conditional constraints.

**Hub-spoke validation**:
```bash
./doc/scripts/validate.py hub-spoke
```

Checks bidirectional relationships between hubs and spokes, including 100% back-reference compliance.

**Template validation**:
```bash
./doc/scripts/validate.py template doc/_meta/02-templates/hub.md
```

Validates template frontmatter against `validation_schema.json` and checks template-specific rules.

**Full validation**:
```bash
./doc/scripts/validate.py validate-all
```

Runs all validation checks in sequence.

### Exit Codes

- **0**: All validation passed
- **1**: Validation failures detected

**CI integration**: CI/CD pipeline runs `validate-all` and fails build on non-zero exit.

## Validation Error Messages

Validation errors include file path, line number, rule violated, expected value, and actual value.

**Example error**:
```
[ERROR] doc/06-security/authentication.md:1: title_pattern
  Detail: Title does not match required pattern
  Expected: Title ending with "Implementation"
  Found: "# Authentication Guide"
```

**Severity levels**:
- **error**: Blocks commit
- **warn**: Informational only

**Troubleshooting**: See `doc/_meta/04-tooling/validation-dsl-reference.md` Troubleshooting section for debugging validation failures.

## Standards Evolution

Standards evolve through documented governance procedures.

### Adding New Standards

1. Propose standard in governance review
2. Document standard in appropriate spoke document
3. Update templates with new validation rules
4. Update `validation_schema.json` if DSL changes needed
5. Announce change in documentation standards hub

### Breaking Changes

Breaking changes to validation DSL increment schema_version.

**Migration procedure**:
1. Implement new schema_version validator
2. Support both old and new versions during transition
3. Update all templates to new schema_version
4. Deprecate old schema_version
5. Remove old version support after migration complete

**Current version**: 1 (no breaking changes since initial release)

## Related Documents

**Dependencies**:
- **README.md**: Hub document providing standards overview
- **hub-and-spoke-architecture.md**: Hub creation and structure requirements
- **frontmatter-reference.md**: Frontmatter field definitions and constraints
- **claude-md-format.md**: CLAUDE.md format specification

**References**:
- **doc/_meta/02-templates/README.md**: Template usage including validation examples
- **doc/_meta/04-tooling/validation-dsl-reference.md**: Complete DSL syntax reference
- **doc/_meta/04-tooling/architecture.md**: Validation system implementation
- **doc/scripts/validation_schema.json**: JSON Schema for validation blocks

**Extended by**:
- **doc/_meta/03-governance/quarterly-review.md**: Procedures for updating standards
- **doc/_meta/03-governance/hub-consolidation.md**: Hub creation governance
