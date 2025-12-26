---
doc_type: hub
status: active
date_created: 2025-11-07
primary_category: validation
consolidated_spokes:
  - responsibility-matrix.md
  - ui-validation.md
  - api-validation.md
  - input-sanitization.md
  - runtime-validation.md
  - database-validation.md
tags:
  - validation
  - input-sanitization
  - security
---

# Validation Architecture

## Context

Data validation concepts currently appear across seven ADRs without a unified strategy. This fragmentation creates critical security gaps in input sanitization, validation responsibility ambiguity across layers, and contradicts the centralized validation principle. The current pain points include no comprehensive input sanitization specifications for security-critical operations, unclear responsibility boundaries between UI, API, Runtime, and Database layers, validation logic scattered across multiple documents, missing guidance on critical security controls (UTF-8 validation, HTML escaping, SQL injection prevention), and inconsistent error handling strategies across system layers.

This hub consolidates validation implementation strategy from domain-specific ADRs while architectural principles document the validation philosophy. The architectural principles establish the philosophical foundation (validate early in development, validate once in production, debug vs release mode strategy, centralization principle); this hub provides the complete validation architecture with layer-specific responsibilities, security specifications, and error handling standards.

## Decision

We will implement a **unified validation strategy** with clearly defined responsibilities across four layers (UI, API, Runtime, Database), comprehensive coverage of 12 validation types, and explicit input sanitization specifications following OWASP security guidelines.

This document serves as the validation hub providing strategic overview of TrapperKeeper's validation architecture. It consolidates validation considerations from authentication, API design, rule expressions, type systems, field resolution, schema evolution, and configuration management into a cohesive strategy with cross-references to detailed implementation documents.

### Validation Layers and Responsibilities

Validation occurs at exactly four distinct layers, each with specific responsibilities.

**1. UI Layer (Web Browser, Server-Side Rendering)**:

- HTML5 form validation before submission
- Server-side form validation using go-playground/validator
- Inline error messages with ARIA accessibility
- Prevention of invalid combinations through UI controls (e.g., disable incompatible operator/field_type combinations)
- Minimize JavaScript per simplicity principle; focus on server-side validation

**2. API Layer (web-ui or sensor-api endpoints)**:

- Primary enforcement point for all validation rules
- Early validation before expensive operations (e.g., database writes)
- Pre-compile rules at sync time (not per-record)
- Structured error responses with HTTP status codes (422 for validation errors, 400 for malformed requests)
- Input sanitization for all user-provided data

**3. Runtime Layer (SDK-Side and Server-Side Processing)**:

Enforce validation during rule evaluation in both SDK execution (client library) and server processing (web-ui and sensor-api services). Different validation types apply to different scopes.

- Enforce validation during rule evaluation and event processing
- Type coercion and conversion
- Field path resolution and missing field detection
- Apply on_missing_field policies
- Buffer and resource limit enforcement

**4. Database Layer (SQLite or PostgreSQL constraints)**:

- Type constraints on columns (INTEGER, TEXT, TIMESTAMP)
- Foreign key constraints for referential integrity
- Unique indexes on identifiers (rule_id, user_id, api_key_id)
- NOT NULL constraints where applicable
- No CHECK constraints (maintained in application code due to backend variations)

**Responsibility Matrix**: Each validation type specifies which layer is responsible for enforcement. See Responsibility Matrix for consolidated layer assignment view with SDK/Server scope clarifications, and layer-specific spoke documents for complete implementation details.

**Cross-References**:

- Responsibility Matrix: Complete 12×4 layer assignment matrix with SDK/Server scope markers
- UI Validation: Complete UI layer specifications
- API Validation: API layer enforcement patterns
- Runtime Validation: Runtime validation rules with SDK vs server scope
- Database Validation: Database constraint specifications

### Validation Centralization Principle

Validation logic is centralized to ensure consistency across all system components.

**Centralization Architecture**:

- **internal/types**: Core domain models and type definitions
- **internal/rules**: Runtime validation functions, rule parsing, execution logic
- **sdks/go**: Reuses internal/rules validation for SDK-side enforcement
- **cmd/sensor-api**: Reuses internal/rules validation for server-side enforcement
- **cmd/web-ui**: Reuses internal/rules validation for form validation
- **UI templates**: Server-side validation using internal/rules through API layer

The internal package architecture establishes clear boundaries: `internal/rules` contains shared validation logic consumed by both client SDKs and server commands without creating circular dependencies. This centralization eliminates duplication and ensures identical validation behavior across SDK, API, and Web UI.

**Cross-References**:

- Architectural Principles: Data Validation Vision (centralization philosophy)
- Client/Server Separation: Package boundaries and validation ownership

### Validation Types and Layer Distribution

The validation strategy covers 12 distinct validation types, each with explicit layer responsibility distribution.

**12 Validation Types** (see spoke documents for complete details):

1. **Structural Format Validation**: UUID format, timestamp format/precision, field path syntax
2. **Rule Expression Validation**: Expression syntax, operator/field-type compatibility, nested wildcards (max 2), field_ref no-wildcard rule, on_missing_field enum, sample_rate range (0.0-1.0), array homogeneity
3. **Type Coercion and Conversion**: Numeric/text/boolean coercion, type mismatch detection, null vs coercion failure distinction
4. **Resource Limits**: Event buffer (128 count, 1MB size, 128MB total), metadata (64 pairs, 64KB total), tag length (128 chars), nested depth limits
5. **Configuration and Startup Validation**: Config value types (boolean, integer, duration, port, path, URL, log level), dependencies, secrets in files rejection, database connection, CLI arguments
6. **Web Form Input Validation**: Form field lengths, email format, proper HTTP status codes (422 vs 400), CSRF token validation
7. **Authentication and Credentials Validation**: Password hashing (bcrypt), API key format (tk-v1-{id}-{random}), HMAC-SHA256 signature, session expiry (24h)
8. **TLS and Transport Security Validation**: Certificate validation (expiry, key match), timestamp drift (<100ms warning)
9. **Data Integrity Validation**: Migration checksums (SHA256), migration sequence tracking
10. **Runtime Field Resolution Validation**: Missing field detection, type mismatch handling, on_missing_field policy enforcement
11. **Performance and Cost Validation**: Rule cost budget validation (<1ms target), sampling rate validation
12. **Input Sanitization (SECURITY-CRITICAL)**: UTF-8 validation (reject 0x00-0x1F except tab/LF/CR), HTML escaping (html/template), SQL injection prevention (database/sql parameterized queries), command injection prevention (never spawn processes), path traversal prevention (canonicalize paths)

**Layer Distribution Pattern**: Each validation type specifies UI/API/Runtime/Database enforcement with explicit SDK vs server scope where applicable.

**Cross-References**:

- API Validation: Complete validation type specifications with examples
- Runtime Validation: SDK vs server scope matrix

### Input Sanitization Specifications (SECURITY-CRITICAL)

Security-critical input sanitization controls enforced at the API layer as primary security boundary following OWASP guidelines.

**Five Core Security Controls**: UTF-8 validation with control character filtering, HTML escaping via html/template, SQL injection prevention via database/sql parameterized queries, command injection prevention (architectural constraint), and path traversal prevention via canonicalization.

For complete input sanitization specifications with implementation patterns, Go code examples, validation domain architecture clarifications (Domain 1 strict validation vs Domain 2 best-effort sanitization), layer responsibility matrix, attack examples, and OWASP compliance checklist, see Input Sanitization.

**Cross-References**:

- Input Sanitization: Complete OWASP sanitization patterns and implementation details
- API Validation: API layer enforcement patterns
- Runtime Validation: Defense-in-depth re-validation patterns

### Error Handling Standards Per Layer

Each layer implements consistent error handling strategies appropriate to its role in the system.

**Four Layer Error Handling Patterns**:

1. **UI Layer Error Handling**: Server-side rendering with inline error messages and ARIA accessibility. Inline field-level errors displayed adjacent to form fields. Error summary at top of form listing all validation failures. HTTP 422 responses from API trigger form re-render with errors. Preserve user input on validation failure.

2. **API Layer Error Handling**: Structured JSON responses with HTTP status codes. 422 Unprocessable Entity for validation errors (well-formed request, invalid data). 400 Bad Request for malformed requests (invalid JSON, missing required fields). 401 Unauthorized for authentication failures. 500 Internal Server Error for unexpected server errors (never expose internal details).

3. **Runtime Layer Error Handling**: Apply on_missing_field policy for missing fields; type coercion failures treated as condition failed. Missing fields: apply on_missing_field policy (skip/fail/match per rule configuration). Type coercion failures: condition evaluates to false, continue evaluation (NOT missing field). Log validation failures with full context (field path, actual value, expected type). Debug mode validates assumptions frequently; release mode validates once when data loaded.

4. **Database Layer Error Handling**: User-friendly error messages; fail fast with no retry logic. Convert database errors to user-friendly messages (never expose raw SQL errors). Unique constraint violations: "Resource already exists". Foreign key violations: "Referenced resource not found". Connection failures: service fails to start; no automatic retry (fail fast principle). Transaction failures: rollback and return error; no automatic retry.

**Cross-References**:

- API Validation: Complete error response specifications
- Runtime Validation: Error handling patterns with SDK vs server scope
- Resilience Hub: Error handling strategy across all error categories

### Prevention Mechanisms

Prevention mechanisms enforce validation early in the request pipeline to minimize expensive operations and improve user experience.

**API Layer Prevention (PRIMARY FOCUS)**:

1. **Pre-Compilation at Sync Time**: Validate and compile rules when created/updated (not per-record)
2. **Input Sanitization First**: Validate UTF-8 and control characters before any processing
3. **Format Validation Second**: Validate UUIDs, timestamps, field paths before database operations
4. **Business Logic Validation Third**: Validate operator/field_type compatibility, resource limits, cost budgets
5. **Database Operation Last**: Only write to database after all validation passes

**UI Layer Prevention (SERVER-SIDE)**:

- Disable Invalid Combinations: If user selects `gt` operator, field_type dropdown shows only `any`, `numeric`
- HTML5 Validation: required, maxlength, pattern attributes on form fields
- Server-Side Re-Validation: Always re-validate on server even with HTML5 validation (defense in depth)
- Dynamic Dropdown Filtering: Hide invalid options based on current form state (future consideration: requires JavaScript)

**Cross-References**:

- API Validation: Complete validation flow with examples
- UI Validation: Form validation and prevention patterns

## Consequences

**Benefits**:

- Single source of truth for all validation strategy decisions
- Clear responsibility matrix eliminates ambiguity about layer enforcement
- Security by design through comprehensive input sanitization specifications following OWASP guidelines
- Centralized validation logic in tk-types library ensures consistency across SDK, API, and Web UI
- Fail fast approach prevents cascading failures and improves error messages
- Testable validation requirements enable comprehensive test coverage
- Maintainable through centralized validation logic reducing duplication

**Trade-offs**:

- Increased complexity: 12 validation types and 4 layers increase cognitive load for new developers
- Performance overhead: Multi-layer validation adds latency (mitigated by early rejection and pre-compilation)
- False positives: Strict UTF-8 control character filtering may reject legitimate use cases (tab/LF/CR allowed as compromise)
- Server-side rendering constraints: Minimal JavaScript limits real-time validation feedback (accepted tradeoff per simplicity principle)
- Centralization coupling: tk-types library becomes critical dependency; changes require coordination across components

## Related Documents

**Consolidated Spokes** (this hub consolidates):

- Responsibility Matrix: 12×4 matrix showing validation type assignments across all layers with SDK/Server scope markers
- UI Validation: UI Layer validation specifications and form validation patterns
- API Validation: API Layer enforcement, structured error responses, complete validation type specifications
- Input Sanitization: OWASP security controls (UTF-8, HTML escaping, SQL injection, path traversal) with validation domain architecture
- Runtime Validation: Runtime Layer validation with explicit SDK vs server scope matrix
- Database Validation: Database Layer constraint specifications

**Dependencies** (foundational documents):

- Architectural Principles: Data Validation Vision establishes centralized validation philosophy
- Database Backend: database/sql parameterized queries for SQL injection prevention
- Web Framework: html/template for HTML escaping and CSRF protection

**References** (related hubs/documents):

- Security Hub: Input sanitization integrates with overall security architecture
- Resilience Hub: Error handling strategy complements validation error handling
- Rule Expression Language: Rule expression validation integrates with rule semantics
- Type System: Type coercion validation integrates with type semantics

**Extended by**:

- Testing Strategy: Validation test coverage requirements
