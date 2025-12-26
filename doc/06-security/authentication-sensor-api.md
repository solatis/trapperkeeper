---
doc_type: spoke
status: active
date_created: 2025-11-07
primary_category: security
hub_document: /Users/lmergen/git/trapperkeeper/doc/06-security/README.md
tags:
  - authentication
  - sensor-api
  - hmac
  - api-keys
  - grpc
---

# Sensor API Authentication

## Context

The Sensor API requires authentication for sensors and SDK clients accessing rule synchronization and event ingestion endpoints via gRPC. This document specifies HMAC-based API key authentication for long-running automated clients.

**Hub Document**: This document is part of the [Security Architecture](README.md). See [Security Hub](README.md) Section 2 for strategic overview of dual authentication strategy and why HMAC-based authentication is appropriate for automated sensors with zero-downtime requirements.

## HMAC-Based API Key Authentication

TrapperKeeper uses **HMAC-SHA256 signatures** with structured API key format for fast, stateless authentication.

### API Key Format

Structured format enabling O(1) secret lookup without trial-and-error:

```
tk-v1-550e8400e29b41d4a716446655440000-d7ed499a8f7efd6e6252cf3416788ed8d038b01d4c39d6e62eb6f775c59ca112
│   │  │                               └─────────────────────────── Random data (256 bits / 64 hex chars)
│   │  └─────────────────────────────────────────────────────────── HMAC Secret ID (UUID / 32 hex chars)
│   └────────────────────────────────────────────────────────────── Version (v1)
└────────────────────────────────────────────────────────────────── Prefix (tk-)
```

**Format Components**:

- **Total Length**: 102 characters
- **Prefix**: `tk-` for identification and secret scanning tools
- **Version**: `v1` enables future format changes without breaking existing keys
- **HMAC Secret ID**: UUID (32 hex chars) identifying which HMAC secret validates this key
- **Random Data**: 256-bit random hex string (64 chars) providing entropy

**Benefits**:

- O(1) secret lookup (no trial-and-error across multiple secrets)
- Multiple active secrets supported simultaneously (rotation support)
- Stateless verification (key contains all needed information)
- Clean migration path during rotation (old keys continue working)

### HMAC Secret Bootstrapping

Two deployment patterns supported via **dual-mode bootstrapping**:

**Production Pattern (Environment Variables)**:

- Check `TK_HMAC_SECRET` for single secret (no rotation)
- Check `TK_HMAC_SECRET_1`, `TK_HMAC_SECRET_2`, etc. for rotated secrets
- Multiple numbered variables enable graceful rotation:
  - Deploy with `TK_HMAC_SECRET_1` (old) + `TK_HMAC_SECRET_2` (new)
  - Generate new API keys using `TK_HMAC_SECRET_2`
  - Roll out new keys to sensors
  - Remove `TK_HMAC_SECRET_1` once all sensors migrated
- Secrets stored in external systems (Kubernetes secrets, AWS Secrets Manager, etc.)

**Development Pattern (Auto-Generation)**:

- If no `TK_HMAC_SECRET*` variables set: auto-generate 256-bit secret on first boot
- Store generated secret in database (`hmac_secrets` table)
- Load from database on subsequent restarts
- Enables immediate startup for evaluation/testing

**Rationale**: Delegating secrets management to external systems follows production best practices (SOC2 compliance requirement), while auto-generation provides zero-configuration developer experience.

### HMAC Secret Storage

**Loading Strategy**:

1. On startup, parse all `TK_HMAC_SECRET*` environment variables
2. For each environment secret:
   - Hash with SHA256 (not bcrypt - performance requirement)
   - Upsert into `hmac_secrets` with `source="environment"`
3. If no environment secrets found:
   - Check database for auto-generated secret
   - If none exists: generate new 256-bit secret, store with `source="auto-generated"`
4. Build in-memory lookup map: `secret_id → secret_hash`

**Database Schema**:

```sql
CREATE TABLE hmac_secrets (
  secret_id UUID PRIMARY KEY,           -- UUIDv7 identifier for the secret
  secret_hash BLOB NOT NULL,            -- SHA256 hash of the raw secret
  source TEXT NOT NULL,                 -- "environment" or "auto-generated"
  created_at TIMESTAMP NOT NULL,
  CONSTRAINT valid_source CHECK (source IN ('environment', 'auto-generated'))
);

CREATE INDEX idx_hmac_secrets_source ON hmac_secrets(source);
```

**Rationale**:

- Hashing secrets before database storage protects against database compromise
- SHA256 (not bcrypt) chosen for performance (verification happens on every API call)
- Storing secret ID enables efficient O(1) lookup during authentication

### API Key Lifecycle

**Provisioning**:

- Web UI only (CLI/automatic out of scope for MVP)
- Users assign human-readable names to keys for identification
- Generate 256-bit cryptographically secure random data
- Select HMAC secret (defaults to most recent)
- Compute HMAC-SHA256 hash: `HMAC-SHA256(hmac_secret, full_api_key)`
- Store hash in database with UUIDv7 identifier
- Return full key to user **once** (unrecoverable if lost)

**Database Schema**:

```sql
CREATE TABLE api_keys (
  api_key_id UUID PRIMARY KEY,          -- UUIDv7 identifier
  tenant_id UUID NOT NULL,
  name TEXT NOT NULL,                   -- User-assigned name
  key_hash BLOB NOT NULL,               -- HMAC-SHA256 hash of full key
  secret_id UUID NOT NULL,              -- FK to hmac_secrets
  created_at TIMESTAMP NOT NULL,
  last_used_at TIMESTAMP,
  revoked_at TIMESTAMP,
  FOREIGN KEY (secret_id) REFERENCES hmac_secrets(secret_id),
  FOREIGN KEY (tenant_id) REFERENCES tenants(tenant_id)
);

CREATE INDEX idx_api_keys_tenant ON api_keys(tenant_id);
CREATE INDEX idx_api_keys_secret ON api_keys(secret_id);
CREATE INDEX idx_api_keys_revoked ON api_keys(revoked_at) WHERE revoked_at IS NULL;
```

**Rotation**:

- Manual through Web UI (no automatic deprecation in MVP)
- Users create new key, deploy it to sensors, then revoke old key
- Allow 2-3 active keys per tenant for controlled overlap
- Performance optimization: `last_used_at` updated max once per minute (reduces write overhead)

**Revocation**:

- Mark key as revoked in database (soft delete)
- Revoked keys return `PERMISSION_DENIED` gRPC status
- Revoked keys remain in database for audit trail

### Authentication Flow

**Request Authentication** (gRPC metadata `x-api-key`):

1. Extract API key from `x-api-key` metadata
2. Parse format: `tk-v1-<secret-id>-<random-data>`
3. Validate format (reject if malformed without database lookup)
4. Lookup HMAC secret using `secret-id` (O(1) from in-memory map)
5. Compute HMAC-SHA256 hash of full key
6. Query database: `SELECT key_hash, revoked_at FROM api_keys WHERE secret_id = ?`
7. Compare computed hash against stored hash (constant-time comparison)
8. Check revocation status (`revoked_at IS NULL`)
9. Update `last_used_at` if >1 minute since last update (throttled for performance)
10. Proceed with request or return error

**Implementation Pseudocode**:

```go
func authenticateRequest(md metadata.MD) (string, error) {
    // Step 1: Extract API key from metadata
    apiKeys := md.Get("x-api-key")
    if len(apiKeys) == 0 {
        return "", ErrMissingKey
    }
    apiKey := apiKeys[0]

    // Step 2: Parse format
    parts := strings.Split(apiKey, "-")
    if len(parts) != 4 || parts[0] != "tk" || parts[1] != "v1" {
        return "", ErrInvalidFormat
    }
    secretID := parts[2]   // 32 hex chars
    randomData := parts[3] // 64 hex chars

    // Step 3: Lookup HMAC secret (O(1) from in-memory map)
    hmacSecret, ok := secretMap[secretID]
    if !ok {
        return "", ErrUnknownKey
    }

    // Step 4: Compute HMAC-SHA256 hash
    computedHash := hmacSHA256(hmacSecret, apiKey)

    // Step 5: Compare against stored hash
    var stored struct {
        KeyHash   []byte
        TenantID  string
        RevokedAt *time.Time
        APIKeyID  string
        LastUsedAt *time.Time
    }
    err := db.QueryRow(
        "SELECT key_hash, tenant_id, revoked_at, api_key_id, last_used_at FROM api_keys WHERE secret_id = ?",
        secretID,
    ).Scan(&stored.KeyHash, &stored.TenantID, &stored.RevokedAt, &stored.APIKeyID, &stored.LastUsedAt)
    if err != nil {
        return "", err
    }

    if !hmac.Equal(computedHash, stored.KeyHash) {
        return "", ErrInvalidKey
    }

    // Step 6: Check revocation
    if stored.RevokedAt != nil {
        return "", ErrKeyRevoked
    }

    // Step 7: Update last_used_at (throttled)
    if shouldUpdateLastUsed(stored.LastUsedAt) {
        _ = db.Exec(
            "UPDATE api_keys SET last_used_at = NOW() WHERE api_key_id = ?",
            stored.APIKeyID,
        ) // Non-critical, ignore errors
    }

    // Step 8: Return tenant ID for request context
    return stored.TenantID, nil
}

func shouldUpdateLastUsed(lastUsed *time.Time) bool {
    if lastUsed == nil {
        return true // Never used, always update
    }
    return time.Since(*lastUsed) > time.Minute
}
```

**Error Responses** (gRPC status codes):

- Missing API key: `UNAUTHENTICATED` - "API key required in x-api-key metadata"
- Invalid format: `UNAUTHENTICATED` - "Invalid API key format"
- Unknown key: `UNAUTHENTICATED` - "Invalid API key"
- Revoked key: `PERMISSION_DENIED` - "API key has been revoked"

**Security Monitoring**:

- Log all authentication failures with client IP address
- Enable detection of brute-force attempts or compromised keys
- Exclude sensitive data (no key values in logs)

### HMAC vs Bcrypt for API Keys

**Decision**: Use HMAC-SHA256, not bcrypt.

**Rationale**:

- **Performance**: Authentication happens on every gRPC call (potentially thousands per second per sensor)
- **Sufficient Security**: 256-bit random data provides strong entropy, HMAC-SHA256 is cryptographically secure
- **Different Threat Model**: API keys shown once and stored by clients (not memorized like passwords)
- **Bcrypt Purpose**: Intentionally slow to resist brute-force against weak passwords, unnecessary overhead for high-entropy API keys

**Key Point**: Bcrypt appropriate for user passwords (Web UI), HMAC appropriate for API keys (Sensor API). Different authentication contexts require different algorithms.

## Edge Cases and Limitations

**Known Limitations**:

- Single recovery point: Keys shown once at creation, unrecoverable if lost (must revoke and regenerate)
- No automatic key expiration or deprecation warnings in MVP
- No granular permissions: All keys have full sensor permissions (read rules + write events)
- Secret storage risk: Auto-generated secrets stored in database (mitigated by SHA256 hashing)
- Race conditions: Multiple environment variable secrets loaded on startup may have timing issues (non-concern for MVP single-instance deployment)

**Edge Cases**:

- Key rotation with zero downtime: Both old and new keys valid during migration window (multiple numbered environment variables)
- Last_used_at updates: Throttled to 1-minute intervals, may not reflect exact latest usage (acceptable for audit purposes)
- Database backup: Auto-generated secrets stored in database must be included in backups (hashed, but required for key validation)

## Related Documents

**Dependencies** (read these first):

- [API Service Architecture](../../02-architecture/api-service.md): gRPC metadata handling, stateless protocol
- [Configuration Management](../../09-operations/configuration.md): TK_HMAC_SECRET environment variable enforcement, secrets rejection in config files
- [Identifiers (UUIDv7)](../03-data/identifiers-uuidv7.md): UUIDv7 for api_key_id and secret_id identifiers

**Related Spokes** (siblings in this hub):

- [Authentication (Web UI)](authentication-web-ui.md): Contrasts HMAC-based (Sensor API) vs cookie-based (Web UI) authentication
- [Configuration Security](configuration-security.md): HMAC secret environment variable requirements
- [Encryption Strategy](encryption.md): HMAC-SHA256 key management lifecycle (Section 2.2, Section 4.2)

**Extended by**:

- [Validation Hub](../07-validation/README.md): API key format validation (tk-v1-{id}-{random} pattern, HMAC signature verification)
- [Testing Philosophy](../01-principles/testing-philosophy.md): HMAC-based API authentication in integration test scenarios
