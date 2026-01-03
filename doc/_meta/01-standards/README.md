---
doc_type: hub
status: active
primary_category: documentation
consolidated_spokes:
  - hub-and-spoke-architecture.md
  - frontmatter-reference.md
  - claude-md-format.md
  - claude-md-structure.md
  - documentation-standards.md
tags:
  - meta-documentation
  - standards
  - validation
maintainer: Documentation Team
---

# Documentation Standards

## Context

Documentation standards were scattered across multiple documents without a clear organizational structure. Standards for hub-and-spoke architecture, frontmatter metadata, and CLAUDE.md navigation files existed as separate documents without coordination. This made it difficult to understand which standards apply in which contexts and created inconsistencies in how documentation was created and validated.

Standards need to be prescriptive and comprehensive, defining exact requirements for documentation structure, metadata, and navigation. Without a unified standards hub, documentation quality varies across the project, and automated validation cannot enforce consistent patterns.

## Decision

We will establish **comprehensive documentation standards** covering all structural patterns, metadata requirements, and navigation file formats.

This document serves as the standards hub providing prescriptive requirements for creating and maintaining Trapperkeeper documentation. Standards are organized into architectural patterns (hub-and-spoke), metadata schemas (frontmatter), and navigation formats (CLAUDE.md).

### Hub-and-Spoke Architecture Standard

The hub-and-spoke architecture standard defines when and how to create hub documents that consolidate related documentation spokes.

**Key Points:**

- Hub documents (README.md) required when 3+ spokes address common domain
- Hubs provide strategic overview with cross-references to implementation details
- 90%+ back-reference compliance required between hubs and spokes
- Maximum 3 clicks from hub to any spoke detail

**Cross-References:**

- **hub-and-spoke-architecture.md**: Complete hub creation criteria, quality thresholds, navigation requirements, validation rules

**Example**: Security hub consolidates authentication, encryption, and TLS spokes with strategic overview of security architecture.

### Frontmatter Metadata Standard

Frontmatter metadata standard specifies required YAML metadata fields for all documentation enabling discovery and validation.

**Key Points:**

- All documentation requires frontmatter with doc_type, status, primary_category
- Spokes must include hub_document back-reference field
- Hubs must include consolidated_spokes list (minimum 3 for product docs)
- Forbidden fields include version, revision, changelog (use git for history)

**Cross-References:**

- **frontmatter-reference.md**: Field definitions, required vs optional fields, allowed values, validation rules
- **frontmatter-hub.schema.json**: JSON Schema for hub frontmatter validation
- **frontmatter-spoke.schema.json**: JSON Schema for spoke frontmatter validation

**Example**: Hub frontmatter includes `consolidated_spokes: [auth.md, encryption.md, tls.md]` while each spoke includes `hub_document: README.md`.

### CLAUDE.md Navigation Format Standard

CLAUDE.md format standard defines fast-index navigation files that guide LLM agents to relevant documentation.

**Key Points:**

- CLAUDE.md files use trigger pattern "Read when [condition]" for each entry
- No YAML frontmatter (pure markdown navigation index)
- Maximum 50 lines (60 for doc/CLAUDE.md root)
- Forbidden patterns include "how to", "step 1", "contains information", "describes"

**Cross-References:**

- **claude-md-format.md**: Complete format specification, trigger patterns, forbidden content, length limits
- **claude-md-structure.md**: Structural organization patterns, section ordering, consistency requirements

**Example**: `**\`authentication.md\`\*\* - Read when implementing login, session management, or user verification` provides clear trigger condition.

### Validation and Quality Standards

Standards must be enforceable through automated validation to maintain documentation consistency.

**Key Points:**

- All templates must include validation frontmatter with schema_version: 1
- Validation DSL enables template authors to define rules without code changes
- Python validation script validates frontmatter, hub-spoke relationships, CLAUDE.md format, template rules
- 100% validation compliance required for documentation changes
- CI/CD integration prevents merging invalid documentation
- Standards evolve through documented governance procedures

**Cross-References:**

- **documentation-standards.md**: Validation requirements by document type, DSL syntax overview, CI enforcement
- **doc/\_meta/04-tooling/validation-dsl-reference.md**: Complete validation DSL reference with examples
- **doc/\_meta/04-tooling/architecture.md**: Validation implementation details, algorithms, CI/CD integration
- **doc/\_meta/03-governance/hub-consolidation.md**: Procedures for updating standards

**Example**: `validate.py validate-all` checks frontmatter schema, hub-spoke bidirectional links, CLAUDE.md format, template validation rules, producing zero-exit on success.

## Consequences

**Benefits:**

- Consistent documentation structure across all Trapperkeeper docs
- Automated validation enforces standards uniformly
- Clear prescriptive guidance for documentation authors
- LLM agents navigate documentation efficiently with CLAUDE.md
- Frontmatter enables automated discovery and cross-referencing

**Trade-offs:**

- More structured than freeform markdown documentation
- Standards require maintenance as patterns evolve
- Validation adds CI/CD overhead for documentation changes

## Related Documents

**Consolidated Spokes** (this hub consolidates):

- **hub-and-spoke-architecture.md**: Maps to Hub-and-Spoke Architecture Standard subsection
- **frontmatter-reference.md**: Maps to Frontmatter Metadata Standard subsection
- **claude-md-format.md**: Maps to CLAUDE.md Navigation Format Standard subsection
- **claude-md-structure.md**: Maps to CLAUDE.md Navigation Format Standard subsection
- **documentation-standards.md**: Maps to Validation and Quality Standards subsection

**Dependencies** (foundational documents):

- **doc/\_meta/README.md**: Parent meta-documentation hub
- **doc/CLAUDE.md**: Root navigation entry point implementing these standards

**References** (related hubs):

- **doc/\_meta/02-templates/README.md**: Provides templates implementing these standards
- **doc/\_meta/03-governance/README.md**: Defines procedures for maintaining standards
- **doc/\_meta/04-tooling/README.md**: Implements validation automation for standards
