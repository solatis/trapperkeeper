---
doc_type: hub
status: active
date_created: 2025-11-07
primary_category: documentation
consolidated_spokes:
  - hub.md
  - spoke.md
  - claude-md.md
  - cross-cutting-index.md
  - redirect-stub.md
tags:
  - meta-documentation
  - templates
maintainer: Documentation Team
---

# Documentation Templates

## Context

Creating new documentation requires understanding complex structural patterns, frontmatter requirements, and format conventions. Without canonical templates, documentation authors copy-paste from existing docs, perpetuating inconsistencies and missing required elements. Each new document starts from scratch, requiring deep knowledge of standards encoded in separate specification documents.

Templates provide copy-paste starting points with inline guidance explaining each section. They encode standards directly into usable artifacts, reducing cognitive load and ensuring compliance. Templates must balance completeness with usability, providing enough guidance without overwhelming authors.

## Decision

We will provide **canonical templates with inline guidance** for all documentation types: hubs, spokes, CLAUDE.md navigation files, cross-cutting indexes, and redirect stubs.

This document serves as the templates hub providing copy-paste starting points implementing documentation standards. Each template includes frontmatter, section structure, guidance comments, and validation checklists. Authors copy templates, replace placeholders, and delete guidance comments before committing.

### Hub Template

Hub template provides complete structure for consolidating 3+ related spokes into strategic overview document.

**Key Points:**

- Hub template saved as README.md in domain directory
- Includes frontmatter with consolidated_spokes list (minimum 3)
- Provides Context, Decision, Consequences, Related Documents sections
- Contains 3-7 concept area subsections with mandatory structure
- Includes validation checklist covering structural, navigational, content requirements

**Cross-References:**

- **hub.md**: Complete hub template with frontmatter, section markers, guidance comments, validation checklist

**Example**: Copy `hub.md` to `doc/06-security/README.md`, replace placeholders for authentication/encryption/TLS consolidation, delete guidance comments.

### Spoke Template

Spoke template provides structure for implementation detail documents referenced by hubs.

**Key Points:**

- Spoke frontmatter includes hub_document back-reference field
- Provides focused implementation guidance for specific topic
- Cross-references parent hub for strategic context
- Lighter structure than hub (no consolidation requirements)

**Cross-References:**

- **spoke.md**: Complete spoke template with frontmatter, section structure, guidance comments

**Example**: Copy `spoke.md` to `doc/06-security/authentication-web-ui.md`, set `hub_document: README.md`, provide authentication implementation details.

### CLAUDE.md Navigation Template

CLAUDE.md template enforces fast-index format with trigger patterns guiding LLM agents to relevant documentation.

**Key Points:**

- No YAML frontmatter (pure markdown navigation)
- Title format: "# [Text] Guide for LLM Agents"
- Purpose section: 1-3 paragraphs explaining domain
- Hub/Files/Subdirectories sections with "Read when" triggers
- Includes good/bad trigger examples and forbidden patterns

**Cross-References:**

- **claude-md.md**: Complete CLAUDE.md template with format specification, trigger examples, forbidden patterns

**Example**: Copy `claude-md.md` to `doc/06-security/CLAUDE.md`, list authentication.md/encryption.md/tls.md with "Read when" triggers for each.

### Cross-Cutting Index Template

Cross-cutting index template consolidates security, performance, validation, observability, or error-handling concerns across system.

**Key Points:**

- Indexes organize concerns spanning multiple architectural domains
- Provides discovery without duplicating spoke content
- Links to specific sections in spoke documents
- Five canonical indexes: security, performance, validation, observability, error-handling

**Cross-References:**

- **cross-cutting-index.md**: Complete index template with section structure, linking patterns, guidance comments

**Example**: Copy `cross-cutting-index.md` to `doc/security-index.md`, link authentication/encryption/TLS sections from security hub and related spokes.

### Redirect Stub Template

Redirect stub template handles document moves and consolidations, preventing broken links while guiding to new location.

**Key Points:**

- Status set to "superseded" with superseded_by field
- Brief explanation of why document moved
- Clear link to replacement document
- Maintains git history at original location

**Cross-References:**

- **redirect-stub.md**: Complete redirect template with frontmatter, explanation format

**Example**: After consolidating three auth docs into hub, leave redirect stubs pointing to `doc/06-security/README.md`.

## Consequences

**Benefits:**

- Consistent documentation structure from copy-paste templates
- Inline guidance reduces cognitive load for authors
- Templates encode standards directly into usable artifacts
- Validation checklists ensure completeness before committing
- New documentation starts with correct structure and frontmatter

**Trade-offs:**

- Templates require maintenance when standards evolve
- Guidance comments add noise (must be deleted before finalizing)
- May feel prescriptive for experienced documentation authors

## Related Documents

**Consolidated Spokes** (this hub consolidates):

- **hub.md**: Maps to Hub Template subsection
- **spoke.md**: Maps to Spoke Template subsection
- **claude-md.md**: Maps to CLAUDE.md Navigation Template subsection
- **cross-cutting-index.md**: Maps to Cross-Cutting Index Template subsection
- **redirect-stub.md**: Maps to Redirect Stub Template subsection

**Dependencies** (foundational documents):

- **doc/_meta/01-standards/README.md**: Standards that templates implement
- **doc/_meta/README.md**: Parent meta-documentation hub

**References** (related hubs):

- **doc/_meta/03-governance/README.md**: Procedures for using templates during documentation creation
- **doc/_meta/04-tooling/README.md**: Validation tooling that checks template compliance

## Adding Validation to Templates

All templates must include a `validation` block in frontmatter defining structural and content requirements. Validation rules are automatically enforced by `validate.py` during CI checks.

### Minimal Validation Example

Simplest validation with title and length constraints:

```yaml
---
doc_type: template
template_for: simple-doc
status: active
date_created: 2025-11-08
primary_category: documentation

validation:
  schema_version: 1
  title_pattern: "^# .+ Guide$"
  max_lines: 100
---
```

**Use when**: Document has basic structure requirements without complex conditionals.

### Complex Validation Example

Full validation with conditions, frontmatter checks, and section requirements:

```yaml
---
doc_type: template
template_for: claude-md
status: active
date_created: 2025-11-08
primary_category: documentation

validation:
  schema_version: 1

  conditions:
    readme_exists: file_exists("README.md")
    has_md_files: md_files_exist(exclude=["README.md", "CLAUDE.md"])
    has_subdirs: subdirs_exist()

  title_pattern: "^# .+ Guide for LLM Agents$"
  max_lines: 50

  forbidden:
    - pattern: "(?i)how to"
      reason: "how-to instructions belong in implementation docs"
      severity: error

  frontmatter:
    required_fields:
      - doc_type
      - status
      - date_created

  required_sections:
    - name: "Purpose"
      must_exist: true
      max_paragraphs: 3

    - name: "Hub"
      require_if: readme_exists
      content_pattern: '^\*\*`README\.md`\*\* - Read when'

    - name: "Files"
      require_if: has_md_files
      files_rules:
        must_list_all_md: true
        exclude_globs: ["README.md", "CLAUDE.md"]
        entry_pattern: '^\*\*`.+\.md`\*\* - Read when'
---
```

**Use when**: Document has conditional sections, frontmatter requirements, or file listing validation.

### When to Use Each Rule Type

**title_pattern**: Enforce naming conventions (e.g., CLAUDE.md ends with "Guide for LLM Agents").

**max_lines**: Prevent document bloat (CLAUDE.md limited to 50 lines, hubs to 500 lines).

**filename_pattern**: Require specific filenames (hubs must be `README.md`).

**forbidden**: Detect anti-patterns (no "how-to" in navigation files, no version numbers).

**frontmatter**: Validate metadata fields (doc_type enum, date format, required fields).

**required_sections**: Enforce structure (hubs need Context/Decision/Consequences sections).

**conditions + require_if**: Make sections optional based on directory state (Files section only if .md files exist).

**files_rules**: Ensure complete inventories (CLAUDE.md lists all documentation files).

### Validation DSL Reference

For complete syntax, predicates, and troubleshooting, see **doc/_meta/04-tooling/validation-dsl-reference.md**.
