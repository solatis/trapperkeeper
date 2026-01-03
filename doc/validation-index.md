---
doc_type: index
status: active
date_created: 2025-11-07
date_updated: 2025-11-07
primary_category: validation
cross_cutting:
  - validation
maintainer: Validation Team
last_review: 2025-11-07
next_review: 2026-02-07
---

# Validation Index

## Purpose

This index provides navigation to all documentation addressing **validation** across the Trapperkeeper system. Use this as a discovery mechanism for validation-related decisions, input sanitization patterns, and validation layer responsibilities regardless of their primary domain. Validation is critical for security (injection attack prevention), data integrity, and system reliability.

## Quick Reference

| Category               | Description                                                                       | Key Documents                                                                                              |
| ---------------------- | --------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| 4-Layer Matrix         | UI, API, Runtime, Database validation responsibilities                            | [Validation Hub](07-validation/README.md), [Responsibility Matrix](07-validation/responsibility-matrix.md) |
| 12 Validation Types    | Authentication, configuration, field paths, type coercion, rule expressions, etc. | [Validation Hub](07-validation/README.md) Section 3                                                        |
| OWASP Sanitization     | SQL injection, XSS, path traversal, command injection prevention                  | [Input Sanitization](07-validation/input-sanitization.md)                                                  |
| SDK vs Server Scope    | Which validation happens in SDK (client-side) vs server (API/database)            | [Responsibility Matrix](07-validation/responsibility-matrix.md)                                            |
| Centralized Validation | go-playground/validator as standard validation library                            | [Validation Hub](07-validation/README.md) Section 4                                                        |

## Core Concepts

### 4-Layer Responsibility Matrix

TrapperKeeper validation uses a 4-layer responsibility model: UI Layer (user experience, immediate feedback), API Layer (security enforcement, protocol compliance), Runtime Layer (business logic, SDK/server split), and Database Layer (data integrity, schema constraints). Each layer has distinct responsibilities with explicit SDK vs server scope clarifications.

**Relevant Documentation:**

- **[Validation Hub](07-validation/README.md)** - Strategic overview of validation architecture → See Section 2 for complete 4-layer model
- **[Responsibility Matrix](07-validation/responsibility-matrix.md)** - Detailed layer-by-layer validation assignments with SDK/server scope markers for all 12 validation types
- **[Integration Overview](10-integration/README.md)** - How validation logic distributes across internal/rules, sdks/go/, internal/core packages

### 12 Validation Types

TrapperKeeper validation addresses 12 distinct validation concerns: Authentication Credentials, Configuration Values, Rule Expression Syntax, Rule Expression Semantics, Field Path Syntax, Field Path Resolution, Type Coercion, Missing Fields, API Keys, Web Form Input, Database Constraints, and Rule Complexity Limits. Each validation type has specific layer responsibilities and implementation patterns.

**Relevant Documentation:**

- **[Validation Hub](07-validation/README.md)** - Complete enumeration of 12 validation types → See Section 3 for type-by-type specifications
- **[Responsibility Matrix](07-validation/responsibility-matrix.md)** - Layer assignments for each validation type with scope clarifications
- **[Input Sanitization](07-validation/input-sanitization.md)** - OWASP sanitization patterns for web form input and API payloads

### OWASP Input Sanitization

All user input follows OWASP sanitization guidelines to prevent SQL injection, XSS, path traversal, and command injection attacks. Specific sanitization patterns: parameterized SQL queries (database layer), HTML auto-escaping with html/template (UI layer), path canonicalization (API layer), and input length limits (all layers).

**Relevant Documentation:**

- **[Input Sanitization](07-validation/input-sanitization.md)** - Detailed OWASP sanitization specifications for all input types → See Section 2 for injection attack prevention
- **[Validation Hub](07-validation/README.md)** - Input sanitization overview → See Section 5 for OWASP integration
- **[Security Hub](06-security/README.md)** - Security implications of input validation → See Section 5 for injection defense

### SDK vs Server Validation Scope

Validation logic splits between SDK (client-side) and server (API/database) based on trust boundaries and implementation constraints. SDKs perform rule expression syntax/semantics validation, field path syntax validation, and type coercion validation. Server performs authentication, authorization, configuration, database constraints, and security-critical validation. Scope clarifications prevent duplication and ensure consistent validation across SDK languages.

**Relevant Documentation:**

- **[Responsibility Matrix](07-validation/responsibility-matrix.md)** - SDK/SERVER markers for all 12 validation types -> See Runtime Layer rows for scope assignments
- **[Validation Hub](07-validation/README.md)** - SDK vs server validation rationale -> See Section 2.3 for trust boundary analysis
- **[Integration Overview](10-integration/README.md)** - How internal/rules package enables shared validation logic across SDKs -> See Section 4 for validation architecture

### Centralized Validation with go-playground/validator

All validation logic uses go-playground/validator as the standard validation library, shared by Go SDK (sdks/go/) and server (internal/core) via internal/rules. Single source of truth eliminates duplication, ensures consistent validation across SDK languages, and simplifies maintenance. Validation includes rule expression syntax/semantics, field path syntax, type coercion, and rule complexity limits.

**Relevant Documentation:**

- **[Validation Hub](07-validation/README.md)** - Centralized validation architecture -> See Section 4 for validation library responsibilities
- **[Integration Overview](10-integration/README.md)** - Package design and dependency graph -> See Section 3 for validation centralization
- **[Rule Expression Language](04-rule-engine/expression-language.md)** - How validation integrates with rule compilation -> See Section 3 for syntax validation

### Configuration Validation

Configuration validation enforces security constraints (no secrets in files), type correctness (ports are integers), range constraints (timeouts are positive), and required field presence. Validation happens at startup using viper or environment variables, rejecting invalid configurations before services start. Fail-fast approach prevents insecure or broken deployments.

**Relevant Documentation:**

- **[Validation Hub](07-validation/README.md)** - Configuration validation overview -> See Section 3.2 for validation rules
- **[Configuration Security](06-security/configuration-security.md)** - Security-specific configuration validation -> See Section 2 for secrets enforcement
- **[Configuration Management](09-operations/configuration.md)** - How validation integrates with multi-source configuration -> See Section 4 for startup validation

### Authentication Credential Validation

Authentication validation prevents credential stuffing, enforces password complexity, rate-limits login attempts, and validates session token integrity. Web UI validation uses bcrypt password hashing with complexity requirements (min 8 chars, mixed case, numbers, symbols). API validation uses HMAC-SHA256 signature verification with replay attack prevention.

**Relevant Documentation:**

- **[Validation Hub](07-validation/README.md)** - Authentication validation overview → See Section 3.1 for credential validation rules
- **[Web Authentication](06-security/authentication-web-ui.md)** - Cookie-based authentication validation → See Section 2 for password complexity enforcement
- **[API Authentication](06-security/authentication-sensor-api.md)** - HMAC-based API key validation → See Section 3 for signature verification

### Field Path and Type Validation

Field path validation enforces syntax correctness (`a.b.c` structure), nested wildcard limits (max 2 levels), and reserved namespace protection (`$tk.*` prefix). Type coercion validation ensures safe conversions between field types, fails gracefully on invalid coercion, and provides detailed error messages. Validation happens at rule compilation time, preventing invalid rules from reaching production.

**Relevant Documentation:**

- **[Validation Hub](07-validation/README.md)** - Field path and type validation → See Section 3.5 (field paths) and Section 3.7 (type coercion)
- **[Field Path Resolution](04-rule-engine/field-path-resolution.md)** - Field path syntax and validation → See Section 1 for syntax grammar
- **[Type System and Coercion](04-rule-engine/type-system-coercion.md)** - Type coercion validation rules → See Section 2 for safe coercion matrix
- **[Optimization Strategies](05-performance/optimization-strategies.md)** - Nested wildcard validation limits → See Section 3 for performance constraints

### Database Constraint Validation

Database validation enforces schema constraints (NOT NULL, UNIQUE, FOREIGN KEY), data type constraints (INTEGER, TIMESTAMP, VARCHAR), and referential integrity. Validation happens at database layer using native SQL constraints, providing defense-in-depth beyond API validation. Failed validation produces structured errors with constraint violation details.

**Relevant Documentation:**

- **[Validation Hub](07-validation/README.md)** - Database validation overview → See Section 2.4 for database layer responsibilities
- **[Database Backend](09-operations/database-backend.md)** - Schema constraints and enforcement → See Section 3 for constraint definitions
- **[Database Migrations](09-operations/database-migrations.md)** - How schema changes affect validation → See Section 4 for constraint migration

## Domain Coverage Matrix

| Domain         | Coverage | Key Document                                                                   |
| -------------- | -------- | ------------------------------------------------------------------------------ |
| Architecture   | ✓        | [Integration Overview](10-integration/README.md)                               |
| API Design     | ✓        | [API Service](02-architecture/api-service.md)                                  |
| Database       | ✓        | [Database Backend](09-operations/database-backend.md)                          |
| Security       | ✓        | [Security Hub](06-security/README.md)                                          |
| Performance    | ✓        | [Performance Hub](05-performance/README.md) (validation cost)                  |
| Validation     | ✓        | [Validation Hub](07-validation/README.md)                                      |
| Configuration  | ✓        | [Configuration Security](06-security/configuration-security.md)                |
| Testing        | ✓        | [Testing Philosophy](01-principles/testing-philosophy.md) (validation testing) |
| Deployment     | ✗        | N/A (deployment is operational concern)                                        |
| Error Handling | ✓        | [Error Taxonomy](08-resilience/error-taxonomy.md) (validation errors)          |

## Patterns and Best Practices

### Defense-in-Depth Validation

**Description**: Validation implemented at multiple layers—UI (user experience), API (security enforcement), Runtime (business logic), Database (data integrity). Each layer provides independent validation; compromise or bypass of one layer does not compromise system integrity. Layered validation catches different error classes at appropriate boundaries.

**Used In**:

- [Validation Hub](07-validation/README.md) Section 2
- [Responsibility Matrix](07-validation/responsibility-matrix.md)
- [Security Hub](06-security/README.md) Section 7

### Fail-Fast Validation

**Description**: Validation errors are detected and reported at the earliest possible layer. Rule syntax errors detected at compilation time, not runtime. Configuration errors detected at startup, not during request processing. Fail-fast approach prevents invalid data from propagating through system layers.

**Used In**:

- [Validation Hub](07-validation/README.md) Section 6
- [Configuration Management](09-operations/configuration.md) Section 4
- [Rule Expression Language](04-rule-engine/expression-language.md) Section 3

### Centralized Validation Logic

**Description**: Validation rules centralized in internal/rules package, shared by SDKs and server. Single source of truth eliminates duplication, ensures consistency across SDK languages, and simplifies maintenance. Centralization enables validation logic reuse across trust boundaries.

**Used In**:

- [Validation Hub](07-validation/README.md) Section 4
- [Integration Overview](10-integration/README.md) Section 3
- [Responsibility Matrix](07-validation/responsibility-matrix.md)

### OWASP Sanitization Pattern

**Description**: All user input sanitized following OWASP guidelines. Parameterized SQL queries prevent SQL injection. HTML auto-escaping prevents XSS. Path canonicalization prevents path traversal. Input length limits prevent buffer overflow. Sanitization happens at layer boundaries (UI input, API payloads, database queries).

**Used In**:

- [Input Sanitization](07-validation/input-sanitization.md)
- [Validation Hub](07-validation/README.md) Section 5
- [Security Hub](06-security/README.md) Section 5

### Structured Validation Errors

**Description**: Validation errors provide structured information: which field failed, what constraint was violated, what value was provided. Structured errors enable meaningful user feedback (UI layer), security logging (API layer), and debugging (all layers). Error structure consistent across validation types.

**Used In**:

- [Validation Hub](07-validation/README.md) Section 7
- [Error Taxonomy](08-resilience/error-taxonomy.md) Category 6
- [Input Sanitization](07-validation/input-sanitization.md) Section 3

## Related Indexes

- **[Security Index](security-index.md)**: Validation is a critical security control. See security index for injection attack defenses, authentication validation, and secrets enforcement.
- **[Performance Index](performance-index.md)**: Validation has performance implications. See performance index for validation cost considerations and rule complexity limits.
- **[Error Handling Index](error-handling-index.md)**: Validation produces errors. See error handling index for validation error taxonomy and structured error reporting.

## Maintenance Notes

**Last Updated**: 2025-11-07
**Last Review**: 2025-11-07
**Next Review**: 2026-02-07 (quarterly)
**Maintainer**: Validation Team

**Known Gaps**:

- PII detection validation (future consideration, see [Encryption Strategy](06-security/encryption.md) Section 7)
- Rate limiting validation (DDoS protection out of scope for MVP)
- Advanced validation patterns (custom validators, plugin architecture)

**Planned Additions**:

- Validation testing framework (property-based testing for validation rules)
- Validation performance benchmarks (cost of each validation layer)
- Validation error analytics (which validation rules trigger most often)
