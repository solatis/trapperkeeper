---
doc_type: hub
status: active
date_created: 2025-11-07
date_updated: 2025-11-11
primary_category: documentation
consolidated_spokes:
  - hub-consolidation.md
  - cross-cutting-index-governance.md
  - documentation-evolution-principle.md
  - template-maintenance.md
tags:
  - meta-documentation
  - governance
  - procedures
maintainer: Documentation Team
---

# Documentation Governance

## Context

Documentation maintenance requires clear procedures for creating, updating, and consolidating documents. Without governance, documentation grows organically leading to fragmentation, duplication, and inconsistency. Decisions about when to create hubs versus spokes, how to maintain cross-cutting indexes, and when to refactor documentation lack clear criteria.

Governance must balance structure with agility, especially for a 5-person team. Heavyweight governance creates bureaucracy, while no governance creates chaos. Procedures need clear triggers, responsibilities, and workflows without excessive overhead.

## Decision

We will establish **lightweight governance procedures** for documentation creation, maintenance, and evolution suited for small teams.

This document serves as the governance hub defining procedures, workflows, and responsibilities for maintaining Trapperkeeper documentation. Governance covers hub consolidation criteria, cross-cutting index maintenance, and documentation evolution principles.

### Hub Consolidation Governance

Hub consolidation governance defines when to create hubs, how to consolidate spokes, and maintenance workflows.

**Key Points:**

- Create hub when 3+ spokes address common domain (product docs)
- Every _meta/ subdirectory requires hub regardless of spoke count
- Quarterly hub freshness reviews check alignment with spokes
- 90%+ back-reference compliance required
- Hub creation involves consolidation planning, spoke analysis, strategic synthesis

**Cross-References:**

- **hub-consolidation.md**: Complete consolidation procedures, thresholds, workflows, review schedules, quality criteria

**Example**: When authentication, encryption, and TLS docs reach critical mass, create security hub consolidating strategic approach with spoke cross-references.

### Cross-Cutting Index Governance

Cross-cutting index governance defines ownership, update triggers, and conflict resolution for indexes spanning multiple domains.

**Key Points:**

- Five canonical indexes: security, performance, validation, observability, error-handling
- Indexes updated when spoke documents change in relevant domains
- Clear ownership assignments prevent neglect and conflicts
- Automation assists with index validation and freshness checks

**Cross-References:**

- **cross-cutting-index-governance.md**: Index ownership assignments, update triggers, conflict resolution procedures, automation strategy

**Example**: Security index owner updates links when new authentication spoke added to security hub, validated through CI/CD.

### Documentation Evolution Principle

Documentation evolution principle guides how documentation changes over time balancing completeness with agility.

**Key Points:**

- Start with spokes for new domains
- Consolidate into hubs when patterns emerge
- Refactor when fragmentation creates pain
- Git provides history (no revision logs in documents)
- Prefer evolution over big-bang rewrites

**Cross-References:**

- **documentation-evolution-principle.md**: Evolution philosophy, refactoring triggers, incremental improvement strategies

**Example**: Start with individual ADRs for security topics, consolidate into security hub after 3+ related documents emerge.

### Template Maintenance

Template maintenance defines procedures for creating new documentation templates with validation rules and updating existing template validation.

**Key Points:**

- Templates define structure and validation rules for document types
- Validation rules use declarative DSL in template frontmatter
- Step-by-step procedures for creating new templates
- Guidelines for updating existing validation rules
- Complexity management and testing procedures

**Cross-References:**

- **template-maintenance.md**: Complete procedures for template creation, validation updates, testing workflows, complexity limits

**Example**: When creating API reference template, define frontmatter schema, required sections, and validation rules; test with fixtures before committing.

## Consequences

**Benefits:**

- Clear procedures for documentation creation and maintenance
- Lightweight governance suited for 5-person team
- Quarterly reviews prevent documentation drift
- Cross-cutting indexes maintained through clear ownership
- Evolution principle prevents over-documentation and under-documentation extremes

**Trade-offs:**

- Governance requires periodic review time investment
- Procedures add structure that may feel constraining
- Clear ownership means accountability for index maintenance

## Related Documents

**Consolidated Spokes** (this hub consolidates):

- **hub-consolidation.md**: Maps to Hub Consolidation Governance subsection
- **cross-cutting-index-governance.md**: Maps to Cross-Cutting Index Governance subsection
- **documentation-evolution-principle.md**: Maps to Documentation Evolution Principle subsection
- **template-maintenance.md**: Maps to Template Maintenance subsection
- **adr-migration-map.md**: Maps to ADR Migration Mapping subsection

**Dependencies** (foundational documents):

- **doc/_meta/01-standards/README.md**: Standards that governance procedures maintain
- **doc/_meta/README.md**: Parent meta-documentation hub

**References** (related hubs):

- **doc/_meta/02-templates/README.md**: Templates used during governed procedures
- **doc/_meta/04-tooling/README.md**: Validation tooling enforcing governance policies
