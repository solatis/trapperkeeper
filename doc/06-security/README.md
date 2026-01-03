---
doc_type: hub
status: active
date_created: 2025-11-07
primary_category: security
consolidated_spokes:
  - authentication-web-ui.md
  - authentication-sensor-api.md
  - tls-https-strategy.md
  - configuration-security.md
  - encryption.md
related_docs:
  - ../09-operations/configuration.md
  - ../05-performance/README.md
related_hubs:
  - 07-validation
  - 08-resilience
---

# Security Architecture

## Context

Security decisions span multiple implementation domains—web authentication, API authentication, transport security, encryption, and configuration—creating fragmentation that obscures the unified security posture. This fragmentation presents three critical risks:

1. **Incomplete threat model**: No single document articulates which attacker capabilities TrapperKeeper defends against
2. **SOC2 compliance gaps**: Security controls scattered across 6+ documents make audit preparation difficult
3. **Inconsistent implementation**: Developers must synthesize security strategy from authentication spokes, transport (TLS/HTTPS Strategy), encryption, configuration, and validation documents

The dual authentication strategy (cookie-based Web UI vs HMAC Sensor API) is documented separately without explaining why unification is NOT a goal. Security boundaries between services, trust assumptions for single-tenant deployment, and defense-in-depth layering lack cohesive documentation.

## Decision

We will implement **comprehensive security architecture** using defense-in-depth principles with clearly documented security boundaries, dual authentication strategies serving fundamentally different client types, and SOC2-aligned controls.

This document serves as the security hub providing strategic security overview with cross-references to detailed implementation documents. Security is critical for SOC2 Type 2 compliance and production deployments.

### 1. Threat Model and Security Boundaries

TrapperKeeper explicitly defines in-scope and out-of-scope threats to focus security efforts appropriately.

**Attacker Capabilities Defended Against:**

- Network eavesdropping: TLS 1.3 encrypts all transport for both HTTP (Web UI) and gRPC (Sensor API)
- Credential theft: Bcrypt password hashing and HMAC-SHA256 API key signatures prevent plaintext exposure
- Unauthorized API access: Session-based authentication (Web UI) and API key authentication (Sensor API) enforce access control
- Injection attacks: Parameterized SQL queries, HTML auto-escaping (html/template), path canonicalization prevent SQL injection, XSS, and path traversal
- CSRF attacks: Double-submit cookie pattern protects state-changing operations

**Attacker Capabilities NOT Defended Against (Out of Scope for MVP):**

- Database encryption at rest (file system permissions provide baseline protection)
- Physical server access (operator responsibility)
- Supply chain attacks (dependency verification out of scope)
- Advanced persistent threats (APT)
- DDoS attacks (rate limiting not implemented)

**Security Assumptions:**

- Single-tenant deployment: No multi-tenancy security isolation required
- Operators have physical and system access to server infrastructure
- Database credentials managed securely by operator
- TLS certificates provisioned and managed by operator
- Services run on same server and trust each other

**Key Points:**

- Threat model scoped to network-level attacks and credential compromise
- Single-tenant architecture simplifies trust boundaries (services trust database, no cross-service authentication)
- Physical security and infrastructure-level controls are operator responsibilities

**Cross-References:**

- [Authentication (Web UI)](authentication-web-ui.md): Cookie-based session authentication with bcrypt
- [Authentication (Sensor API)](authentication-sensor-api.md): HMAC-based API key authentication
- [TLS/HTTPS Strategy](tls-https-strategy.md): Transport encryption for both services
- [Input Validation](../07-validation/README.md): SQL injection, XSS, CSRF, path traversal prevention

**Example**: Network eavesdropping mitigated by TLS 1.3 for both HTTP (port 8080) and gRPC (port 50051). Credential theft mitigated by storing only bcrypt hashes (passwords) and HMAC-SHA256 signatures (API keys), never plaintext.

### 2. Dual Authentication Strategy

TrapperKeeper implements **two distinct authentication approaches** serving fundamentally different client types. This dual-strategy approach is intentional and will not be unified.

**Task-to-Document Navigation:**

| Task                                | Primary Document                                            | Supporting Reads                                              |
| ----------------------------------- | ----------------------------------------------------------- | ------------------------------------------------------------- |
| Understand why dual authentication? | [Security Hub](README.md) Section 2                         | None (complete rationale in this section)                     |
| Implement Web UI authentication     | [Authentication (Web UI)](authentication-web-ui.md)         | [TLS/HTTPS Strategy](tls-https-strategy.md) (cookie security) |
| Implement Sensor API authentication | [Authentication (Sensor API)](authentication-sensor-api.md) | [Configuration Security](configuration-security.md) (secrets) |

**Why Two Different Approaches:**

| Aspect                  | Web UI (Humans)                | Sensor API (Machines)         |
| ----------------------- | ------------------------------ | ----------------------------- |
| Client lifecycle        | Interactive sessions           | Long-running processes        |
| Re-authentication       | Users can re-login             | Sensors cannot re-login       |
| Session duration        | Short (24 hours of inactivity) | Long (until revoked)          |
| State management        | Cookies in browser             | API key in configuration      |
| Authentication overhead | Bcrypt acceptable              | HMAC required for performance |
| Credential recovery     | Password reset flows           | Revoke and regenerate         |

**Key Points:**

- Different authentication mechanisms serve different clients using industry best practices for each domain
- Cookie-based sessions appropriate for human-interactive Web UI (short-lived, re-login acceptable)
- HMAC API keys appropriate for automated sensors (long-lived, zero-downtime operation critical)
- Attempting to unify would compromise either user experience or operational reliability

**Cross-References:**

- [Authentication (Web UI)](authentication-web-ui.md): Complete cookie-based session authentication specification
- [Authentication (Sensor API)](authentication-sensor-api.md): Complete HMAC-SHA256 API key authentication specification
- [Encryption Strategy](encryption.md): Password hashing (bcrypt) and API key signature (HMAC) algorithms

**Example**: Web UI user logs in with username/password, receives 24-hour session cookie, can re-login when session expires. Sensor configures API key once, uses indefinitely without re-authentication, revokes and regenerates if compromised.

### 3. Transport Security (TLS 1.3)

Both HTTP (Web UI) and gRPC (Sensor API) use TLS 1.3 in production with flexible deployment options.

**Web UI Transport Security:**

- Three deployment modes: HTTPS (direct TLS termination), HTTP + Reverse Proxy (X-Forwarded-Proto detection), HTTP Development Mode
- TLS 1.3 minimum with modern cipher suites (TLS_AES_256_GCM_SHA384, TLS_AES_128_GCM_SHA256, TLS_CHACHA20_POLY1305_SHA256)
- Secure cookie flag dynamically controlled by TLS mode or X-Forwarded-Proto header
- Manual certificate provisioning with validation on startup (Let's Encrypt integration out of scope)

**Sensor API Transport Security:**

- TLS 1.3 for gRPC communication on port 50051
- Separate certificates from Web UI (managed via same procedures)
- HMAC authentication complements TLS (defense in depth)
- TLS prevents network eavesdropping; HMAC prevents unauthorized API access even if TLS compromised

**Key Points:**

- Both HTTP (port 8080) and gRPC (port 50051) use TLS 1.3 in production with separate certificates
- Defense in depth: TLS protects transport, authentication protects access
- Flexible deployment supports direct HTTPS, reverse proxy, and development scenarios

**Cross-References:**

- [TLS/HTTPS Strategy](tls-https-strategy.md): Complete deployment mode specifications, certificate management procedures, X-Forwarded-Proto middleware
- [Authentication (Web UI)](authentication-web-ui.md) Section on Cookie Security: Secure flag integration with TLS modes

**Example**: Production deployment runs HTTPS mode with user-provided certificates (TLS 1.3, TLS_AES_256_GCM_SHA384 cipher). Development deployment runs HTTP mode with localhost binding and secure cookie flag disabled.

### 4. Input Validation and Sanitization

Comprehensive input validation strategy prevents injection attacks and ensures data integrity across four layers.

**Security-Critical Controls:**

- **UTF-8 Validation** (API Layer): All user input validated for UTF-8 encoding, control characters 0x00-0x1F rejected (except tab/LF/CR), null bytes explicitly rejected per OWASP Input Validation Cheat Sheet
- **HTML Escaping** (UI Layer): html/template with auto-escaping enabled, context-appropriate escaping (HTML, JavaScript, URL), never manual HTML construction
- **SQL Injection Prevention** (Database Layer): sqlx parameterized queries exclusively, never format!() or string concatenation for SQL, code review enforcement
- **CSRF Protection** (Web UI): Double-submit cookie pattern for all state-changing operations (POST/PUT/DELETE), CSRF token in hidden form field + cookie, validation before processing
- **Path Traversal Prevention** (API Layer): Path canonicalization before file operations, directory boundary validation, reject paths containing ".." or absolute paths from user input

**Four Validation Layers:**

1. **UI Layer**: HTML5 validation, server-side form validation, inline error messages
2. **API Layer**: Primary enforcement point, early validation before expensive operations, structured error responses
3. **Runtime Layer**: Type coercion, field path resolution, on_missing_field policy enforcement
4. **Database Layer**: Type constraints, foreign keys, unique indexes, NOT NULL constraints

**Key Points:**

- Defense in depth: Multiple independent security controls across layers
- API layer is primary enforcement point (Web UI and Sensor API validate all input)
- Runtime layer handles type coercion and field resolution failures
- Database layer provides final constraint enforcement

**Cross-References:**

- [Validation Hub](../07-validation/README.md): Complete unified validation strategy with 12 validation types and 4-layer responsibility matrix
- [Configuration Security](configuration-security.md): Secrets rejection in config files, environment variable enforcement

**Example**: Web UI form submits rule with field path `metadata.tenant.id == "acme"`. API layer validates UTF-8 encoding (no null bytes), validates field path syntax (no path traversal), validates operator/field_type compatibility. Runtime layer performs type coercion during evaluation. Database layer enforces foreign key constraint on rule_id.

### 5. Secret Management and Configuration Security

Sensitive values (HMAC secrets, database passwords, API keys) are restricted to environment variables and CLI arguments, never configuration files.

**Configuration Constraints:**

- Secrets MUST be provided via environment variables or CLI arguments
- Secrets NEVER stored in configuration files (validation rejects on startup)
- Three-tier precedence: file < env < CLI (CLI arguments override environment variables override config files)

**Secret Lifecycle Coverage:**

- **HMAC Secrets**: Environment variables (`TK_HMAC_SECRET`, `TK_HMAC_SECRET_1`, `TK_HMAC_SECRET_2` for rotation), auto-generated 256-bit secrets in development, dual-secret rotation for zero-downtime migration
- **Passwords**: Bcrypt with cost factor 12, each password uniquely salted, no separate key management required
- **API Keys**: HMAC-SHA256 signatures stored, plaintext keys shown once at creation (unrecoverable if lost)
- **TLS Private Keys**: File system storage with restrictive permissions (600), separate keys for HTTP and gRPC

**Key Points:**

- Configuration security prevents accidental secret leakage in version control
- HMAC secret rotation enables zero-downtime API key migration
- TLS certificates and keys managed separately per service
- Development auto-generation provides zero-configuration experience

**Cross-References:**

- [Configuration Security](configuration-security.md): Complete secrets rejection rules, environment variable policies, validation enforcement
- [Encryption Strategy](encryption.md): Key management lifecycle for bcrypt, HMAC, TLS
- [Authentication (Sensor API)](authentication-sensor-api.md) Section on HMAC Secret Bootstrapping: Dual-mode production vs development

**Example**: Production deployment sets `TK_HMAC_SECRET=<256-bit secret>` environment variable. Service validates at startup that configuration file does NOT contain HMAC secret. If secret found in config file, startup fails with error: "Secret values cannot be stored in configuration files. Use TK_HMAC_SECRET environment variable instead."

### 6. Data Encryption Strategy

Application-layer encryption for credentials and session data maintains database backend flexibility.

**What IS Encrypted:**

- **User Passwords**: Bcrypt with cost factor 12, unique salt per password, hash stored in database
- **API Key Secrets**: HMAC-SHA256 signatures stored, plaintext keys never persisted
- **Session Tokens**: scs library encrypts session data before database storage
- **Transport**: TLS 1.3 for both HTTP (Web UI) and gRPC (Sensor API)

**What is NOT Encrypted (with Rationale):**

- **Database at Rest**: File system permissions protect database file; encryption at rest requires database-specific features conflicting with backend flexibility (SQLite vs PostgreSQL)
- **Event Storage**: JSONL files not encrypted (temporary MVP backend, file system permissions provide baseline protection)
- **Configuration Files**: Secrets forbidden in files per policy; non-sensitive settings don't require encryption

**Key Points:**

- Application-layer encryption works identically across SQLite and PostgreSQL
- Credentials protected by cryptographic hashing (bcrypt, HMAC)
- Transport encryption prevents network eavesdropping
- File system permissions provide baseline data-at-rest protection

**Cross-References:**

- [Encryption Strategy](encryption.md): Complete encryption architecture with key management lifecycle
- [Authentication (Web UI)](authentication-web-ui.md) Section on Password Hashing: Bcrypt implementation details
- [Authentication (Sensor API)](authentication-sensor-api.md) Section on HMAC vs Bcrypt: Performance rationale for API keys

**Example**: User password "SecureP@ssw0rd" hashed with bcrypt cost factor 12, hash `$2b$12$<salt><hash>` stored in database (60 characters). Original password never stored. API key `tk-v1-<secret-id>-<random>` hashed with HMAC-SHA256, 32-byte signature stored in database. Original key shown once at creation, unrecoverable.

### 7. SOC2 Compliance Mapping

High-level mapping to SOC2 Trust Service Criteria for certification preparation.

**Current Controls Documented:**

- **CC6.1 (Logical Access Controls)**: Web UI authentication (cookie-based sessions, bcrypt), Sensor API authentication (HMAC API keys)
- **CC6.6 (Transport Security)**: TLS 1.3 for both HTTP and gRPC, modern cipher suites, certificate validation
- **CC6.7 (Data Encryption)**: Transport encryption (TLS 1.3), credential encryption (bcrypt, HMAC), session encryption (scs)

**Documented Gaps (Mitigations Provided):**

- Database at rest not encrypted: Mitigated by file system permissions (600 or 640), single-tenant deployment, operator physical security responsibility
- Event storage not encrypted: Mitigated by file system permissions, restricted data directory, single-tenant deployment
- Configuration files not encrypted: By design, secrets forbidden in files per [Configuration Security](configuration-security.md) and [Configuration Management](../09-operations/configuration.md)

**Key Points:**

- Architecture designed with SOC2 compliance in mind
- Operational controls and evidence required for certification (logs, configurations, procedures)
- Security monitoring and incident response out of scope for MVP

**Cross-References:**

- [Encryption Strategy](encryption.md) Section on SOC2 Compliance: Complete CC6.7 mapping with audit evidence requirements
- [Authentication (Web UI)](authentication-web-ui.md): CC6.1 compliance for user authentication
- [Authentication (Sensor API)](authentication-sensor-api.md): CC6.1 compliance for API authentication

**Example**: SOC2 audit requires evidence of TLS configuration (certificates, cipher suites, protocol versions), authentication logs (successful logins, failures), and encryption key management procedures (bcrypt cost factor, HMAC secret rotation).

## Consequences

**Benefits:**

- Unified security architecture eliminates fragmentation across multiple documents
- Clear navigation to all security implementation documents
- Explains rationale for dual authentication strategy (prevents future unification attempts)
- Documents security boundaries and trust model for single-tenant architecture
- Maps to SOC2 compliance requirements for certification preparation
- Defense in depth approach with multiple independent security layers
- Clear separation: in-scope vs out-of-scope threats guides security investment

**Trade-offs:**

- Additional hub document adds maintenance burden (must update when security documents change)
- Security overview without implementation details requires navigation to spoke documents
- Single-tenant assumptions limit multi-tenant deployment options
- MVP limitations in authorization, security monitoring, and encryption at rest require future expansion

## Related Documents

**Consolidated Spokes** (this hub consolidates):

- [Authentication (Web UI)](authentication-web-ui.md): Cookie-based sessions, bcrypt, CSRF protection (maps to Section 2)
- [Authentication (Sensor API)](authentication-sensor-api.md): HMAC keys, dual-mode bootstrapping, rotation (maps to Section 2)
- [TLS/HTTPS Strategy](tls-https-strategy.md): Flexible deployment, X-Forwarded-Proto detection, gRPC TLS (maps to Section 3)
- [Configuration Security](configuration-security.md): Secrets in env vars only, validation at startup (maps to Section 5)
- [Encryption Strategy](encryption.md): Application-layer encryption, key management lifecycle, SOC2 mapping (maps to Section 6)

**Dependencies** (foundational documents):

- [API Service Architecture](../02-architecture/api-service.md): gRPC fundamentals, stateless protocol for Sensor API security
- [Architecture Hub](../02-architecture/README.md): Service separation, boundaries, single-tenant trust model
- [Validation Hub](../07-validation/README.md): Complete input sanitization and validation strategy (maps to Section 4)

**References** (related hubs):

- [Validation Hub](../07-validation/README.md): OWASP compliance, injection prevention, 4-layer validation
- [Resilience Hub](../08-resilience/README.md): Security-relevant error handling, authentication failure logging

**Extended by:**

- [Performance Hub](../05-performance/README.md): Validation performance limits, nested wildcard cost analysis for security controls
