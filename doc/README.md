---
doc_type: guide
status: active
date_created: 2025-11-07
primary_category: documentation
---

# TrapperKeeper Documentation Guide

## Purpose

This documentation uses a **hub-and-spoke architecture** optimized for LLM agents and developers navigating complex architectural decisions. Rather than chronological ordering, content is organized by domain with explicit hubs that consolidate related information and provide narrative overviews.

## Quick Navigation

Common queries mapped to documentation paths:

| Query                         | Primary Document                                       | Secondary References                                                                                   |
| ----------------------------- | ------------------------------------------------------ | ------------------------------------------------------------------------------------------------------ |
| How does authentication work? | [Security Hub](06-security/README.md) → Authentication | [Web Auth](06-security/authentication-web-ui.md), [API Auth](06-security/authentication-sensor-api.md) |
| How are rules evaluated?      | [Rule Engine Hub](04-rule-engine/README.md)            | [Expression Language](04-rule-engine/expression-language.md)                                           |
| What's the performance model? | [Performance Hub](05-performance/README.md)            | [Cost Model](05-performance/cost-model.md)                                                             |
| How does validation work?     | [Validation Hub](07-validation/README.md)              | [4-Layer Matrix](07-validation/responsibility-matrix.md)                                               |
| How are errors handled?       | [Resilience Hub](08-resilience/README.md)              | [Error Taxonomy](08-resilience/error-taxonomy.md)                                                      |
| What are the core principles? | [Principles Hub](01-principles/README.md)              | [Testing Philosophy](01-principles/testing-philosophy.md)                                              |
| How is data stored?           | [Data Hub](03-data/README.md)                          | [Event Storage](03-data/event-schema-storage.md), [Identifiers](03-data/identifiers-uuidv7.md)         |
| How do services communicate?  | [Architecture Hub](02-architecture/README.md)          | [API Service](02-architecture/api-service.md)                                                          |
| How is deployment configured? | [Operations Hub](09-operations/README.md)              | [Configuration](09-operations/configuration.md)                                                        |
| How do SDKs integrate?        | [Integration Hub](10-integration/README.md)            | [SDK Model](02-architecture/sdk-model.md)                                                              |

## Domain Overview

TrapperKeeper documentation is organized into 10 domains, each with a hub README providing strategic overview and navigation:

### 1. Principles ([01-principles/](01-principles/README.md))

Core architectural principles: Schema-Agnostic Architecture, Least Intrusive by Default, Ephemeral Sensors, MVP Simplicity, Consistent Encoding, Integration-First Testing. Establishes the philosophical foundation for all design decisions.

**Hub consolidates:** Testing strategy, architectural philosophy

### 2. Architecture ([02-architecture/](02-architecture/README.md))

System architecture decisions: SDK model, service architecture, API service design, binary distribution, package separation, timestamp handling. Covers structural organization and component relationships.

**Key documents:** SDK Model, Service Architecture, API Service, Binary Distribution, Package Separation

### 3. Data ([03-data/](03-data/README.md))

Data management and event handling: Event storage (JSONL), identifiers (UUIDv7), timestamps (Protobuf/chrono), client metadata namespacing. Handles event lifecycle and persistence.

**Key documents:** Event Storage, Identifiers, Timestamps, Client Metadata

### 4. Rule Engine ([04-rule-engine/](04-rule-engine/README.md))

Rule expression system: DNF-based language, field path resolution with wildcards, type system with coercion, schema evolution handling, lifecycle management. Core evaluation logic.

**Hub consolidates:** Expression language, field resolution, type coercion, schema evolution, lifecycle

### 5. Performance ([05-performance/](05-performance/README.md))

Performance optimization strategies: Cost model, sampling techniques, batch processing, nested wildcard limits. Ensures <1ms evaluation targets.

**Hub consolidates:** Cost calculation algorithm, operator costs, field type multipliers, sampling strategies

### 6. Security ([06-security/](06-security/README.md))

Security architecture and controls: Dual authentication (cookie-based Web UI, HMAC Sensor API), TLS 1.3 transport, encryption strategy, input sanitization, SOC2 compliance.

**Hub consolidates:** Threat model, authentication strategies, transport security, encryption, configuration security

### 7. Validation ([07-validation/](07-validation/README.md))

Unified validation strategy: 4-layer responsibility matrix (UI/API/Runtime/Database), 12 validation types, OWASP input sanitization, centralized validation using go-playground/validator.

**Hub consolidates:** Validation concerns from authentication, configuration, rule expressions, field paths, type coercion, missing fields

### 8. Resilience ([08-resilience/](08-resilience/README.md))

Error handling and failure modes: Error taxonomy (6 categories), fail vs degrade vs retry patterns, logging standards (slog), monitoring strategy.

**Hub consolidates:** Network errors, type coercion errors, missing field errors, failure mode configurations

### 9. Operations ([09-operations/](09-operations/README.md))

Operational concerns: Configuration management (viper or environment variables), database backend (SQLite/PostgreSQL/MySQL), migrations, CLI design, web framework (stdlib net/http), health endpoints.

**Key documents:** Configuration, Database Backend, Migrations, CLI Configuration, Web Framework

### 10. Integration ([10-integration/](10-integration/README.md))

Integration patterns: SDK integration model, batch processing, vectorization for Pandas/Spark, polyglot bindings.

**Key documents:** Batch Processing, Vectorization Strategies

## Cross-Cutting Indexes

Five cross-cutting concerns span multiple domains. These indexes aggregate related content for rapid discovery:

1. **[Security Index](security-index.md)** - Authentication, encryption, TLS, secrets management, SOC2 compliance
2. **[Performance Index](performance-index.md)** - Cost model, sampling, batch processing, optimization strategies
3. **[Validation Index](validation-index.md)** - 4-layer matrix, 12 validation types, OWASP sanitization
4. **[Observability Index](observability-index.md)** - Logging, tracing, metrics, health endpoints, monitoring
5. **[Error Handling Index](error-handling-index.md)** - Error taxonomy, fail/degrade/retry patterns, resilience strategies

## How to Navigate

### Hub-and-Spoke Pattern

**Hubs** (README.md files) consolidate 3+ related documents with strategic overview and bidirectional navigation. They provide:

- Unified strategic context and rationale
- Cross-cutting concerns and relationships
- Canonical definitions preventing duplication
- Navigation to detailed implementations

**Spokes** (focused component files) provide implementation specifics:

- Detailed technical specifications
- Code examples and configuration
- Domain-specific constraints and tradeoffs
- Back-references to hub for strategic context

### Navigation Examples (3-Click Principle)

#### Example 1: Understanding validation

1. Click: [Validation Hub](07-validation/README.md)
2. Click: [Responsibility Matrix](07-validation/responsibility-matrix.md) section
3. Click: Specific layer (UI/API/Runtime/Database) details

#### Example 2: Understanding security

1. Click: [Security Hub](06-security/README.md)
2. Click: [Authentication Strategy](06-security/README.md#authentication-strategies) section
3. Click: [Web Authentication](06-security/authentication-web-ui.md) or [API Authentication](06-security/authentication-sensor-api.md) details

#### Example 3: Understanding performance

1. Click: [Performance Hub](05-performance/README.md)
2. Click: [Cost Model](05-performance/cost-model.md)
3. Click: Operator costs or field type multipliers section

#### Example 4: Cross-cutting navigation (Security + Validation)

1. Click: [Security Index](security-index.md)
2. Click: Input Sanitization row → [Validation Hub](07-validation/README.md)
3. Click: [Input Sanitization](07-validation/input-sanitization.md) details

### Benefits of This Architecture

- **Bidirectional navigation**: Discover big picture from implementation details, or drill down from strategy to specifics
- **Single source of truth**: Canonical definitions live in hubs, eliminating duplication and drift
- **Cohesion**: Related decisions logically grouped while maintaining focused, readable documents
- **Maintenance**: Updates to cross-cutting concerns happen in one place
- **LLM-optimized**: Domain organization aligns with natural query patterns

## Meta-Documentation

Documentation about documentation lives in [`_meta/`](_meta/README.md):

- **Standards**: Requirements and schemas for creating documentation
- **Templates**: Starter templates for hubs, spokes, and indexes
- **Governance**: Maintenance procedures, hub consolidation criteria, conflict resolution
- **Tooling**: Validation architecture, linters, formatters

When creating, updating, or deleting documentation, consult `_meta/CLAUDE.md` first.

## Document Structure Conventions

All product documentation includes YAML frontmatter with required fields:

```yaml
---
doc_type: hub|spoke|index|guide|reference
status: active|draft|deprecated|superseded
date_created: YYYY-MM-DD
primary_category: architecture|api|database|security|performance|validation|configuration|testing|deployment|error-handling
---
```

See [`_meta/standards/frontmatter-reference.md`](_meta/standards/frontmatter-reference.md) for complete field definitions.

## For LLM Agents

When navigating this documentation:

1. **Start with hubs**: Hub README.md files provide strategic context and navigation
2. **Use cross-cutting indexes**: For concerns spanning multiple domains (security, performance, validation, observability, error-handling)
3. **Follow bidirectional links**: Spokes reference hubs; hubs reference spokes
4. **Trust canonical sources**: Hubs are authoritative for consolidated information
5. **3-click principle**: Maximum 3 clicks from any hub to detailed implementation content
6. **Check frontmatter**: `doc_type` field indicates document role (hub/spoke/index/guide/reference)

## Maintenance

Hub consolidation criteria (see `_meta/governance/hub-consolidation.md`):

- 3+ related documents on same concern
- Single document exceeds 500 lines
- Document referenced by 5+ others
- Document spawned 3+ sub-documents

When ANY threshold is met, create a hub to consolidate and provide strategic overview.
