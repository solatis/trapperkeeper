---
doc_type: index
status: active
date_updated: 2025-11-07
primary_category: security
cross_cutting:
  - security
maintainer: Security Team
last_review: 2025-11-07
next_review: 2026-02-07
---

# Security Index

## Purpose

This index provides navigation to all documentation addressing **security** across the Trapperkeeper system. Use this as a discovery mechanism for security-related decisions, patterns, and implementations regardless of their primary domain. Security is critical for SOC2 Type 2 compliance and production deployments.

## Quick Reference

| Category                  | Description                                                      | Key Documents                                                                                                                                 |
| ------------------------- | ---------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| Threat Model & Boundaries | Attacker capabilities defended against, security assumptions     | [Security Hub](06-security/README.md)                                                                                                         |
| Authentication Strategies | Dual authentication: cookie-based Web UI, HMAC Sensor API        | [Security Hub](06-security/README.md), [Web Auth](06-security/authentication-web-ui.md), [API Auth](06-security/authentication-sensor-api.md) |
| Transport Security        | TLS 1.3 for HTTP and gRPC, certificate management                | [TLS/HTTPS Strategy](06-security/tls-https-strategy.md)                                                                                       |
| Encryption                | Bcrypt passwords, HMAC-SHA256 API keys, secure cookie encryption | [Encryption Strategy](06-security/encryption.md)                                                                                              |
| Input Validation          | OWASP sanitization, 4-layer validation matrix                    | [Validation Hub](07-validation/README.md), [Input Sanitization](07-validation/input-sanitization.md)                                          |

## Core Concepts

### Threat Model and Security Boundaries

TrapperKeeper explicitly defines in-scope and out-of-scope threats to focus security efforts appropriately. The threat model defends against network eavesdropping, credential theft, unauthorized API access, injection attacks, and CSRF attacks. Out-of-scope: database encryption at rest, physical server access, supply chain attacks, APTs, and DDoS mitigation.

**Relevant Documentation:**

- **[Security Hub](06-security/README.md)** - Complete threat model and security boundaries → See Section 1 for attacker capabilities, security assumptions, and trust model
- **[Failure Modes and Degradation](08-resilience/failure-modes.md)** - Security implications of fail-safe vs fail-closed modes
- **[Configuration Security](06-security/configuration-security.md)** - Security constraints preventing secrets in configuration files

### Authentication Strategies

TrapperKeeper implements dual authentication strategies serving fundamentally different client types. Web UI uses session-based cookie authentication with bcrypt password hashing for human users. Sensor API uses HMAC-SHA256 API key signatures for machine-to-machine communication with embedded credentials.

**Relevant Documentation:**

- **[Security Hub](06-security/README.md)** - Strategic overview of dual authentication rationale -> See Section 2 for authentication architecture
- **[Web Authentication](06-security/authentication-web-ui.md)** - Cookie-based authentication implementation for Web UI using stdlib net/http middleware
- **[API Authentication](06-security/authentication-sensor-api.md)** - HMAC-based authentication for gRPC Sensor API with key rotation strategy
- **[Validation Hub](07-validation/README.md)** - Authentication input validation -> See Section 3.1 for credential validation rules

### Transport Security

All network communication uses TLS 1.3 for encryption in transit. Flexible configuration supports direct HTTPS termination, reverse proxy deployments, and HTTP development mode with automatic secure cookie detection via X-Forwarded-Proto headers.

**Relevant Documentation:**

- **[TLS/HTTPS Strategy](06-security/tls-https-strategy.md)** - Complete TLS configuration patterns, certificate management, and deployment modes
- **[Security Hub](06-security/README.md)** - Transport security overview → See Section 3 for TLS requirements and cipher suites
- **[Service Architecture](02-architecture/service-architecture.md)** - How TLS integrates with tk-sensor-api and tk-web-ui services

### Encryption at Rest

Application-layer encryption for credentials (bcrypt password hashing) and session data (secure cookies). HMAC-SHA256 API key signatures prevent plaintext exposure. Database encryption at rest is explicitly NOT implemented in MVP; file system permissions provide baseline protection.

**Relevant Documentation:**

- **[Encryption Strategy](06-security/encryption.md)** - What IS and is NOT encrypted with rationale, key management lifecycle → See Section 2 for bcrypt parameters, Section 3 for HMAC strategy
- **[Security Hub](06-security/README.md)** - Encryption architecture overview → See Section 4 for encryption layering
- **[Database Backend](09-operations/database-backend.md)** - Database security constraints and operator responsibilities

### Input Validation and Sanitization

Comprehensive input validation follows OWASP guidelines with 4-layer responsibility matrix (UI/API/Runtime/Database) and 12 validation types. All user input is sanitized to prevent SQL injection, XSS, and path traversal attacks. Parameterized SQL queries, HTML auto-escaping (html/template), and path canonicalization enforce defense-in-depth.

**Relevant Documentation:**

- **[Validation Hub](07-validation/README.md)** - Unified validation strategy and 4-layer matrix → See Section 2 for OWASP sanitization patterns
- **[Input Sanitization](07-validation/input-sanitization.md)** - Detailed OWASP sanitization specifications for all input types
- **[Security Hub](06-security/README.md)** - Security validation overview → See Section 5 for injection attack defenses
- **[Configuration Security](06-security/configuration-security.md)** - Configuration validation and secrets enforcement

### CSRF Protection

Double-submit cookie pattern protects all state-changing operations in the Web UI. CSRF tokens are generated per-session, validated server-side, and rejected if missing or mismatched. API endpoints are not affected as HMAC authentication is not vulnerable to CSRF.

**Relevant Documentation:**

- **[Web Authentication](06-security/authentication-web-ui.md)** - CSRF token generation and validation -> See Section on CSRF Protection Implementation for double-submit pattern
- **[Security Hub](06-security/README.md)** - CSRF defense strategy -> See Section 1 for CSRF in threat model
- **[Web Framework](09-operations/web-framework.md)** - stdlib net/http middleware integration for CSRF protection

### Secrets Management

Secrets (database credentials, TLS private keys, session signing keys) are NEVER stored in configuration files. They must be provided via environment variables or secure key stores. Configuration validation enforces this constraint at startup, rejecting any configuration containing secret-like patterns.

**Relevant Documentation:**

- **[Configuration Security](06-security/configuration-security.md)** - Secrets enforcement policy and validation rules → See Section 2 for prohibited patterns
- **[Encryption Strategy](06-security/encryption.md)** - Key management lifecycle for bcrypt, HMAC, TLS → See Section 5 for key rotation
- **[Configuration Management](09-operations/configuration.md)** - Three-tier precedence with environment variable override

### SOC2 Compliance Mapping

TrapperKeeper's security architecture maps to SOC2 Trust Service Criteria for Common Criteria (CC) controls. Explicit mapping ensures audit readiness and demonstrates control effectiveness.

**Relevant Documentation:**

- **[Security Hub](06-security/README.md)** - Complete SOC2 compliance mapping → See Section 6 for CC control table
- **[Encryption Strategy](06-security/encryption.md)** - CC6.7: Encryption of sensitive data → See Section 6 for compliance mapping
- **[Validation Hub](07-validation/README.md)** - CC6.1: Input validation controls

## Domain Coverage Matrix

| Domain         | Coverage | Key Document                                                                 |
| -------------- | -------- | ---------------------------------------------------------------------------- |
| Architecture   | ✓        | [Service Architecture](02-architecture/service-architecture.md)              |
| API Design     | ✓        | [API Service](02-architecture/api-service.md)                                |
| Database       | ✓        | [Database Backend](09-operations/database-backend.md)                        |
| Security       | ✓        | [Security Hub](06-security/README.md)                                        |
| Performance    | ✓        | [Performance Hub](05-performance/README.md) (timing attack considerations)   |
| Validation     | ✓        | [Validation Hub](07-validation/README.md)                                    |
| Configuration  | ✓        | [Configuration Security](06-security/configuration-security.md)              |
| Testing        | ✓        | [Testing Philosophy](01-principles/testing-philosophy.md) (security testing) |
| Deployment     | ✓        | [TLS/HTTPS Strategy](06-security/tls-https-strategy.md)                      |
| Error Handling | ✓        | [Error Taxonomy](08-resilience/error-taxonomy.md) (security error handling)  |

## Patterns and Best Practices

### Defense-in-Depth

**Description**: Security controls implemented at multiple layers—network (TLS), authentication (cookies/HMAC), input validation (4 layers), and database (parameterized queries). Each layer provides independent protection; compromise of one layer does not compromise the entire system.

**Used In**:

- [Security Hub](06-security/README.md) Section 7
- [Validation Hub](07-validation/README.md) Section 2
- [Encryption Strategy](06-security/encryption.md) Section 1

### Dual Authentication Strategy

**Description**: Separate authentication mechanisms for fundamentally different client types. Human users (Web UI) use session-based cookies with password authentication. Machine clients (Sensor API) use HMAC-signed API keys. Unification is explicitly NOT a goal due to different security requirements and threat models.

**Used In**:

- [Security Hub](06-security/README.md) Section 2
- [Web Authentication](06-security/authentication-web-ui.md)
- [API Authentication](06-security/authentication-sensor-api.md)

### Configuration Security Enforcement

**Description**: Configuration validation enforces security constraints at startup, rejecting configurations containing secrets, invalid TLS settings, or insecure defaults. Fail-fast approach prevents insecure deployments.

**Used In**:

- [Configuration Security](06-security/configuration-security.md) Section 2
- [Configuration Management](09-operations/configuration.md) Section 4
- [Validation Hub](07-validation/README.md) Section 3.5

### Secure Defaults

**Description**: All security-sensitive settings default to secure values. TLS defaults to enabled in production. Cookies default to HttpOnly, Secure, SameSite=Strict. Default mode is fail-safe: rules disabled when offline, sensor operates as pass-through. Operators must explicitly opt into less secure configurations.

**Used In**:

- [Security Hub](06-security/README.md) Section 7
- [TLS/HTTPS Strategy](06-security/tls-https-strategy.md) Section 2
- [Web Authentication](06-security/authentication-web-ui.md) Section on Cookie Security Configuration

## Related Indexes

- **[Validation Index](validation-index.md)**: Input validation is a critical security control. See validation index for complete 4-layer validation matrix and OWASP sanitization patterns.
- **[Error Handling Index](error-handling-index.md)**: Security errors (authentication failures, authorization denials) follow unified error handling patterns. See error handling index for security error taxonomy.
- **[Observability Index](observability-index.md)**: Security events (authentication failures, authorization denials, suspicious input) are logged with structured tracing. See observability index for security logging patterns.

## Maintenance Notes

**Last Updated**: 2025-11-07
**Last Review**: 2025-11-07
**Next Review**: 2026-02-07 (quarterly)
**Maintainer**: Security Team

**Known Gaps**:

- Database encryption at rest (deferred to post-MVP, see [Encryption Strategy](06-security/encryption.md) Section 7)
- Rate limiting and DDoS protection (out of scope for MVP)
- Advanced threat detection (APT, anomaly detection)

**Planned Additions**:

- API key rotation automation (manual rotation currently documented)
- Security testing framework integration (penetration testing, vulnerability scanning)
- Multi-tenancy security isolation (if multi-tenancy becomes a requirement)
