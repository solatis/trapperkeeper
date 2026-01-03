---
doc_type: spoke
status: active
primary_category: security
hub_document: doc/06-security/README.md
tags:
  - encryption
  - bcrypt
  - hmac
  - tls
  - key-management
---

# Encryption Strategy

## Context

TrapperKeeper requires comprehensive encryption strategy documenting what IS and is NOT encrypted, with rationale for each decision. This document specifies application-layer encryption for credentials and session data, maintaining database backend flexibility (SQLite and PostgreSQL support).

**Hub Document**: This document is part of the [Security Architecture](README.md). See [Security Hub](README.md) Section 6 for strategic overview of data encryption and SOC2 compliance mapping (CC6.7).

## Core Principle: Application-Layer Encryption

**Where Encryption Happens**:

- **Application Runtime**: Encryption performed by tk-sensor-api and tk-web-ui services
- **NOT Database Layer**: Database stores encrypted values but does not perform encryption
- **Rationale**: Avoids lock-in to database-specific encryption features (SQLite encryption extensions, PostgreSQL pgcrypto)

**Database Backend Flexibility**:

- Encryption for passwords and API tokens occurs in application code before database storage
- Database receives only encrypted values (bcrypt hashes, HMAC-SHA256 signatures)
- Works identically with SQLite and PostgreSQL without modification
- Future database migrations do not require re-encryption

## What IS Encrypted

### User Passwords

**Algorithm**: Bcrypt with cost factor 12.

**Implementation**:

- golang.org/x/crypto/bcrypt for password hashing
- Cost factor 12 balances security and performance (~100ms per hash on modern CPU)
- Each password has unique embedded salt (no separate salt storage)

**Storage**:

- Database stores only bcrypt hash output (never plaintext passwords)
- Hash format: `$2b$12$<salt><hash>` (60 characters)

**Location**: Web UI authentication flow (tk-web-ui service).

**Key Management**: No separate keys required (algorithm embeds salt in output).

**Rotation**: Not applicable (each password independently salted).

**Cross-Reference**: [Authentication (Web UI)](authentication-web-ui.md) Section on Password Hashing for complete bcrypt implementation details.

### API Key Secrets

**Algorithm**: HMAC-SHA256 for signature validation.

**Implementation**:

- crypto/hmac and crypto/sha256 from Go standard library for signature computation
- Structured key format: `tk-v1-<secret-id>-<random-data>`
- 256-bit random entropy per key

**Storage**:

- Database stores SHA256 hash of full key (32 bytes)
- Secret IDs stored separately for O(1) lookup
- Plaintext keys never stored (shown once at creation)

**Location**: Sensor API authentication flow (tk-sensor-api service).

**Key Management**: See HMAC Secrets section below for complete lifecycle.

**Rotation**: Dual-secret rotation strategy enables zero-downtime key migration.

**Cross-Reference**: [Authentication (Sensor API)](authentication-sensor-api.md) Section on HMAC-Based API Key Authentication for complete implementation.

### Session Tokens

**Algorithm**: scs library default (cryptographically secure random).

**Implementation**:

- scs manages session ID generation
- Session data encrypted by scs before database storage
- Secure cookie attributes (httpOnly, secure flag, SameSite=Lax)

**Storage**:

- Database stores encrypted session data
- Session IDs in httpOnly cookies (not accessible to JavaScript)

**Location**: Web UI session management (tk-web-ui service).

**Key Management**: scs library manages session encryption keys internally.

**Rotation**: Library handles automatic session key rotation.

**Cross-Reference**: [Authentication (Web UI)](authentication-web-ui.md) Section on Session Lifecycle for complete scs integration.

### Transport Encryption

**Algorithm**: TLS 1.3 with modern cipher suites.

**Implementation**:

- Web UI: Port 8080 (TLS 1.3 in production)
- Sensor API: gRPC TLS on port 50051 (TLS 1.3)
- Cipher suites: TLS_AES_256_GCM_SHA384, TLS_AES_128_GCM_SHA256, TLS_CHACHA20_POLY1305_SHA256
- Separate certificates for each service

**Location**: Both tk-web-ui and tk-sensor-api services.

**Key Management**: See TLS Private Keys section below for complete lifecycle.

**Cross-Reference**: [TLS/HTTPS Strategy](tls-https-strategy.md) for complete transport security implementation.

## What is NOT Encrypted (with Rationale)

### Database at Rest

**Status**: NOT encrypted in MVP.

**Rationale**:

- Requires database-specific features (SQLite: SQLCipher extension, PostgreSQL: pgcrypto or full disk encryption)
- Conflicts with database backend flexibility principle
- Introduces operational complexity (key management, performance overhead)

**Security Mitigation**:

- File system permissions (600 or 640) protect database file
- Single-tenant deployment assumption (operator controls server)
- Physical server security is operator responsibility

**Future Consideration**: Application-level field encryption (database-agnostic) or database-specific encryption when production requirements clarified.

### JSONL Event Storage

**Status**: NOT encrypted in MVP.

**Rationale**:

- Temporary storage backend for MVP only (future replacement with time-series database)
- Event data not considered highly sensitive (internal system telemetry)

**Security Mitigation**:

- File system permissions protect event files
- Events stored in restricted data directory
- Single-tenant deployment limits exposure

**Future Consideration**: Coordinate encryption strategy with time-series database migration.

### Configuration Files

**Status**: NOT encrypted.

**Rationale**:

- Operators responsible for securing configuration files
- Secrets explicitly REJECTED in configuration files (validation enforcement)
- Environment variables or CLI arguments required for secrets
- Configuration files contain only non-sensitive settings

**Security Mitigation**:

- File system permissions protect configuration files
- Configuration validation rejects secrets in configuration at startup
- Sensitive values must use environment variables

**Cross-Reference**: [Configuration Security](configuration-security.md) for complete secrets rejection policy.

### Rule Definitions

**Status**: NOT encrypted.

**Rationale**:

- Not considered sensitive for MVP (internal system rules)
- Rules describe business logic, not customer data
- Single-tenant deployment limits exposure

**Future Consideration**: If rules contain PII detection patterns, consider field-level encryption when multi-tenant support added.

## Key Management Strategy

### Bcrypt Password Hashing

**Key Generation**: N/A (algorithm embeds salt in hash output).

**Key Storage**: N/A (no separate keys).

**Cost Factor Configuration**:

- Cost factor: 12 (configured in application code)
- Provides strong security with acceptable performance (~100ms per hash)
- Cost factor increase requires password re-hash on next login

**Rotation**: Not applicable (each password has unique salt).

**Rationale**: Bcrypt designed to eliminate separate salt management.

### HMAC Secrets

**Generation**:

- Production: Environment variables (`TK_HMAC_SECRET`, `TK_HMAC_SECRET_1`, `TK_HMAC_SECRET_2`)
- Development: Auto-generated 256-bit cryptographically secure random
- SHA256 hash before database storage (not bcrypt - performance requirement)

**Storage**:

- Database stores SHA256(raw_secret), not plaintext (32 bytes)
- `hmac_secrets` table with `secret_id` (UUIDv7 primary key)
- `source` field tracks provenance ("environment" or "auto-generated")

**Rotation**:

- Multiple numbered environment variables enable dual-secret rotation
- Deploy with both old and new secrets active simultaneously
- Generate new API keys using new secret
- Roll out new keys to sensors
- Remove old secret after migration complete

**Lifecycle**:

1. **Generation**: Cryptographically secure 256-bit random (development) or operator-provided (production)
2. **Storage**: SHA256 hashed before database persistence
3. **Rotation**: Dual-secret pattern enables zero-downtime migration
4. **Revocation**: Remove environment variable, restart service (old API keys invalidated)

**Cross-Reference**: [Authentication (Sensor API)](authentication-sensor-api.md) Section on HMAC Secret Bootstrapping for complete implementation.

### Session Secrets

**Generation**: scs library manages session key generation.

**Storage**: Database stores session data encrypted by scs.

**Rotation**: Library handles automatic rotation.

**Lifecycle**: No manual intervention required.

**Rationale**: Delegate session encryption to well-tested library.

### TLS Private Keys

**Generation**:

- Development: Self-signed certificates (openssl, mkcert)
- Production: Certificate authority (Let's Encrypt, internal CA)

**Storage**:

- File system at operator-defined paths
- Separate keys for HTTP (Web UI) and gRPC (Sensor API)
- Example paths: `/etc/trapperkeeper/tls/web-ui-key.pem`, `/etc/trapperkeeper/tls/sensor-api-key.pem`

**Rotation**:

- Manual renewal process requires service restart
- Monitor expiry for BOTH certificates (30-day warning threshold)
- Independent renewal schedules (different expiry dates allowed)

**Lifecycle**:

1. **Generation**: CA-signed or self-signed with openssl/mkcert
2. **Storage**: PEM-encoded files with restrictive permissions (600)
3. **Rotation**: Manual renewal, service restart required
4. **Revocation**: Replace certificate files, restart service

**Cross-Reference**: [TLS/HTTPS Strategy](tls-https-strategy.md) Section on Certificate Management for complete procedures.

### Key Storage Security

**Application-Layer Keys** (HMAC secrets, session secrets):

- Stored in database (protected by file system permissions)
- crypto/sha256 hashed before storage (HMAC secrets only)
- Database credentials managed by operator

**TLS Private Keys**:

- Stored on file system with restrictive permissions (600)
- Operator responsible for securing key files
- Never stored in database

**Database Credentials**:

- Environment variables or secure configuration
- Operator responsibility
- Never in configuration files

**Key Separation**: Each encryption context uses independent keys (no shared key material).

## Defense in Depth Integration

**Layer 1: Network Layer**:

- TLS 1.3 encryption prevents eavesdropping
- Separate certificates for HTTP and gRPC
- Modern cipher suites prevent downgrade attacks

**Layer 2: Authentication Layer**:

- Credentials verified before access (HMAC or sessions)
- Bcrypt password hashing prevents credential theft
- HMAC-SHA256 API key signatures prevent forgery

**Layer 3: Application Layer**:

- Passwords hashed with bcrypt before database storage
- API keys hashed with HMAC before database storage
- Session data encrypted by scs before database storage

**Layer 4: Storage Layer**:

- File system permissions (600 or 640) protect database file
- Database stores only encrypted/hashed values, never plaintext
- Operator physical security responsibility for server access

**Key Principle**: Multiple independent security controls so single failure does not compromise system.

## SOC2 Compliance Mapping

### CC6.7: Data Encryption

**Current Coverage**:

- ✅ Credentials encrypted at rest (bcrypt password hashes, HMAC API key signatures)
- ✅ Transport encryption (TLS 1.3 for both HTTP and gRPC)
- ✅ Session data encrypted by scs
- ✅ Secure cookie attributes (httpOnly, secure flag, SameSite=Lax)

**Documented Gaps**:

- ⚠️ Database at rest not encrypted (mitigated by file system permissions)
- ⚠️ Event data (JSONL) not encrypted at rest (mitigated by file system permissions)
- ⚠️ Configuration files not encrypted (by design, secrets forbidden in files)

**Mitigation Strategy**:

- File system permissions (600 or 640) protect database and event files
- Single-tenant deployment assumption reduces multi-tenant exposure risk
- Physical server security is operator responsibility
- Future: Application-level field encryption or database-specific encryption

**Audit Evidence Requirements**:

- Configuration showing bcrypt cost factor 12
- HMAC secret generation and storage procedures
- TLS certificate validation and renewal procedures
- File system permission policies for database and event files
- Environment variable configuration for production deployments

## Edge Cases and Limitations

**Known Limitations**:

- Database at rest not encrypted (file system permissions provide baseline protection)
- Event storage not encrypted (temporary MVP limitation)
- Manual key rotation procedures (no automation in MVP)
- File system permissions relied upon for data protection
- No PII handling policy in MVP
- Credential storage risk mitigated by hashing but not eliminated

**Edge Cases**:

- Auto-generated HMAC secrets: Stored in database (hashed), required for API key validation (database backup critical)
- Certificate expiry: Services continue running with expired certificates (clients reject TLS handshake)
- Bcrypt cost factor changes: Requires password re-hash on next user login
- HMAC secret rotation: Brief window where both old and new secrets valid (zero-downtime migration)

## Related Documents

**Dependencies** (read these first):

- [Authentication (Web UI)](authentication-web-ui.md): Bcrypt password hashing implementation context
- [Authentication (Sensor API)](authentication-sensor-api.md): HMAC-SHA256 API key authentication implementation context
- [TLS/HTTPS Strategy](tls-https-strategy.md): TLS transport encryption implementation context
- [Configuration Security](configuration-security.md): Secrets via environment variables constraint, config file rejection
- [Database Backend](../09-operations/database-backend.md): Database backend flexibility maintained by application-layer encryption

**Related Spokes** (siblings in this hub):

- [Authentication (Web UI)](authentication-web-ui.md): Implements bcrypt password hashing (Section on Password Hashing)
- [Authentication (Sensor API)](authentication-sensor-api.md): Implements HMAC-SHA256 API key authentication (Section on HMAC Secret Lifecycle)
- [TLS/HTTPS Strategy](tls-https-strategy.md): Implements TLS transport encryption (Certificate Management sections)
- [Configuration Security](configuration-security.md): Implements secrets rejection in config files (Secrets Policy section)

**Extended by**:

- SOC2 audit evidence collection procedures (when implemented post-MVP)
- PII handling policy (when sensor/client API features allow PII detection)
