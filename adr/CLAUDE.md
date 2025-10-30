# TrapperKeeper ADR Index for LLM Consumption

## Purpose
This file provides a machine-parseable index of all Architecture Decision Records (ADRs) in the TrapperKeeper project. It is optimized for LLM agents to quickly understand the ADR structure, dependencies, and relationships.

## Instructions for LLM Agents
1. Parse the structured list below to understand ADR relationships
2. File names now match header ADR numbers (verified as of 2025-10-28)
3. The "Depends on" field indicates ADRs that must be understood first
4. The "Extended by" field indicates ADRs that add detail to this decision
5. ADRs marked "Superseded" are no longer active; refer to their replacement

## Structured ADR List

### ADR-001: Architectural Principles
- File: 001-architectural-principles.md
- Date: 2025-10-28
- Depends on: None (root document)
- Extended by: ADR-002, ADR-003, ADR-004, ADR-005, ADR-021, ADR-024
- Summary: Establishes six core principles - Schema-Agnostic Architecture, Least Intrusive by Default, Ephemeral Sensors, MVP Simplicity, Consistent Encoding/Identifiers, and Integration-First Testing

### ADR-002: SDK Model
- File: 002-sdk-model.md
- Date: 2025-10-28
- Depends on: ADR-001
- Extended by: ADR-023
- Referenced by: ADR-018, ADR-021
- Summary: Developer-first SDK model with ephemeral sensors, pre-compilation for performance, and explicit buffer management

### ADR-003: UUID Strategy
- File: 003-uuid-strategy.md
- Date: 2025-10-28
- Depends on: ADR-001
- Extended by: ADR-019
- Summary: Standardizes on UUIDv7 for all system identifiers, providing time-ordered, globally unique IDs

### ADR-004: Database Backend
- File: 004-database-backend.md
- Date: 2025-10-28
- Depends on: ADR-001
- Extended by: ADR-010
- Summary: Multi-database support with SQLite as default, PostgreSQL/MySQL for production

### ADR-005: API Service Architecture
- File: 005-api-service.md
- Date: 2025-10-28
- Depends on: ADR-001
- Extended by: ADR-006, ADR-012
- Summary: gRPC for sensor communication, stateless protocol with ETAG-based rule sync

### ADR-006: Service Architecture
- File: 006-service-architecture.md
- Date: 2025-10-28
- Depends on: ADR-005
- Extended by: ADR-007, ADR-008, ADR-009, ADR-025
- Summary: Two-service architecture - tk-sensor-api (gRPC) and tk-web-ui (HTTP)

### ADR-007: CLI Configuration
- File: 007-cli-configuration.md
- Date: 2025-10-28
- Depends on: ADR-006
- Summary: Configuration strategy for tk-sensor-api and tk-web-ui services

### ADR-008: Web Framework
- File: 008-web-framework.md
- Date: 2025-10-28
- Depends on: ADR-006
- Extended by: ADR-011
- Summary: Axum web framework for HTTP service layer in tk-web-ui

### ADR-009: Operational Endpoints
- File: 009-operational-endpoints.md
- Date: 2025-10-28
- Depends on: ADR-006
- Summary: Health check endpoints for container orchestration

### ADR-010: Database Migrations
- File: 010-database-migrations.md
- Date: 2025-10-28
- Depends on: ADR-004
- Summary: Explicit migration strategy for multi-database schema changes

### ADR-011: Authentication and Users
- File: 011-authentication-and-users.md
- Date: 2025-10-28
- Depends on: ADR-008
- Related to: ADR-012
- Summary: Cookie-based authentication for Web UI using Axum web framework

### ADR-012: API Authentication
- File: 012-api-authentication.md
- Date: 2025-10-28
- Depends on: ADR-005
- Related to: ADR-011
- Summary: HMAC-based authentication for gRPC sensor API

### ADR-014: Rule Expression Language
- File: 014-rule-expression-language.md
- Date: 2025-10-28
- Depends on: ADR-001
- Extended by: ADR-015, ADR-016, ADR-018
- Summary: DNF-based rule engine with comprehensive operators and performance optimizations

### ADR-015: Field Path Resolution
- File: 015-field-path-resolution.md
- Date: 2025-10-28
- Depends on: ADR-014
- Related to: ADR-016, ADR-017
- Summary: Runtime field path resolution with wildcard support for schema-agnostic evaluation

### ADR-016: Type System and Coercion
- File: 016-type-system-and-coercion.md
- Date: 2025-10-28
- Depends on: ADR-014
- Related to: ADR-015, ADR-017
- Summary: Type coercion rules for rule evaluation with null handling semantics

### ADR-017: Schema Evolution
- File: 017-schema-evolution.md
- Date: 2025-10-28
- Depends on: ADR-015
- Related to: ADR-016
- Summary: Handles missing fields and schema changes with configurable failure modes

### ADR-018: Rule Lifecycle
- File: 018-rule-lifecycle.md
- Date: 2025-10-28
- Depends on: ADR-014, ADR-002
- Related to: ADR-005, ADR-019
- Summary: Rule states (draft/active/disabled), dry-run mode, and operational controls

### ADR-019: Event Schema and Storage
- File: 019-event-schema-and-storage.md
- Date: 2025-10-28
- Depends on: ADR-003, ADR-014
- Extended by: ADR-020
- Summary: JSONL event storage with UUIDv7 IDs and full audit trail

### ADR-020: Client Metadata Namespace
- File: 020-client-metadata-namespace.md
- Date: 2025-10-28
- Depends on: ADR-019
- Related to: ADR-002
- Summary: $tk.* prefix for system metadata in events with size limits

### ADR-021: Failure Modes and Degradation
- File: 021-failure-modes-and-degradation.md
- Date: 2025-10-28
- Depends on: ADR-001, ADR-002
- Related to: ADR-005
- Summary: Fail-safe degradation strategy for network partitions and API failures

### ADR-022: Sampling and Performance Optimization
- File: 022-sampling-and-performance-optimization.md
- Date: 2025-10-28
- Depends on: ADR-014, ADR-015
- Related to: ADR-002, ADR-023
- Summary: Probabilistic sampling and performance optimizations for high-throughput

### ADR-023: Batch Processing and Vectorization
- File: 023-batch-processing-and-vectorization.md
- Date: 2025-10-28
- Depends on: ADR-002, ADR-014
- Related to: ADR-005, ADR-015, ADR-016
- Summary: Vectorized operations for Pandas/Spark data processing frameworks

### ADR-024: Testing Strategy
- File: 024-testing-strategy.md
- Date: 2025-10-29
- Depends on: ADR-001
- Related to: ADR-002, ADR-003, ADR-004, ADR-005, ADR-021
- Summary: Implements Integration-First Testing principle with Testing Trophy model, ephemeral Docker environments, property-based testing, and clear SDK testing boundaries

### ADR-025: Binary Distribution Strategy
- File: 025-binary-distribution.md
- Date: 2025-10-28
- Depends on: ADR-001, ADR-006
- Extended by: ADR-007, ADR-010
- Summary: Single binary with subcommands for unified build and deployment of both services

## Dependency Graph Summary

### Root Documents
- ADR-001: Architectural Principles (foundation for all decisions)

### Core Infrastructure Layer (depends on ADR-001)
- ADR-002: SDK Model
- ADR-003: UUID Strategy
- ADR-004: Database Backend
- ADR-005: API Service Architecture

### Service Layer (depends on core)
- ADR-006: Service Architecture (depends on ADR-005)
- ADR-007: CLI Configuration (depends on ADR-006)
- ADR-008: Web Framework (depends on ADR-006)
- ADR-009: Operational Endpoints (depends on ADR-006)
- ADR-010: Database Migrations (depends on ADR-004)
- ADR-011: Authentication and Users (depends on ADR-008)
- ADR-012: API Authentication (depends on ADR-005)
- ADR-025: Binary Distribution Strategy (depends on ADR-001, ADR-006)

### Rule Engine Layer
- ADR-014: Rule Expression Language (supersedes ADR-001)
- ADR-015: Field Path Resolution (depends on ADR-014)
- ADR-016: Type System and Coercion (depends on ADR-014)
- ADR-017: Schema Evolution (depends on ADR-015)
- ADR-018: Rule Lifecycle (depends on ADR-014, ADR-002)

### Event and Data Layer
- ADR-019: Event Schema (depends on ADR-003, ADR-014)
- ADR-020: Client Metadata (depends on ADR-019)
- ADR-021: Failure Modes (depends on ADR-001, ADR-002)
- ADR-022: Sampling (depends on ADR-014, ADR-015)
- ADR-023: Batch Processing (depends on ADR-002, ADR-014)

### Testing Strategy Layer
- ADR-024: Testing Strategy (depends on ADR-001)

## Maintenance Instructions

### When Adding a New ADR

1. Add entry to "Structured ADR List" section with:
   - File: Path to the ADR file
   - Date: Decision date
   - Depends on: ADRs that must be read first
   - Extended by: ADRs that add detail (if known)
   - Supersedes: If replacing another ADR
   - Summary: One-line description (max 50 words)

2. If the ADR creates a duplicate number:
   - Add canonical name suffix (e.g., ADR-024-topic-name)
   - Add entry to "Duplicate Resolution" section

3. Update "Dependency Graph Summary" if introducing new layer

### When Modifying an Existing ADR

1. Update the Revisions
2. Add ADR references if new ADRs build on it
3. If superseding, mark old ADR as "Superseded" and add "Superseded by"
4. This task MUST be delegated to @agent-adr-writer

### When Removing an ADR

1. Do not delete files unless specifically requested by user
2. Mark Status as "Superseded" or "Deprecated"
3. Update any ADRs that depended on it
