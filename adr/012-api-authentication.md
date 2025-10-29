# ADR-012: API Authentication Strategy

Date: 2025-10-28

## Related Decisions

**Depends on:**
- **ADR-005: API Service Architecture** - Implements authentication for the gRPC sensor API

**Relates to:**
- **ADR-011: Authentication and Users** - Covers Web UI authentication (this ADR covers API only)

## Context

TrapperKeeper's sensor API requires authentication to control access to rules and event ingestion. Key requirements:

- **Fast authentication**: Sensors check rules frequently (every 30 seconds), requiring O(1) verification
- **Zero-configuration development**: Developers should start immediately without complex setup
- **Production-grade security**: Secrets managed externally in production deployments
- **Key rotation support**: Allow graceful migration between old and new keys without downtime
- **Single-tenant MVP**: Authentication scoped to tenant level, full sensor permissions (read rules + write events)
- **Ephemeral sensors**: No persistent sensor identity, keys are shared credentials for tenant access

The authentication strategy must balance developer experience (quick startup) with operational best practices (externalized secrets management).

## Decision

We will implement **HMAC-based API key authentication** with a dual-mode bootstrapping strategy:

### 1. API Key Format

API keys use a structured format that embeds the HMAC secret identifier:

```
tk-v1-550e8400e29b41d4a716446655440000-d7ed499a8f7efd6e6252cf3416788ed8d038b01d4c39d6e62eb6f775c59ca112
│   │  │                               └─────────────────────────── Random data (256 bits / 64 hex chars)
│   │  └─────────────────────────────────────────────────────────── HMAC Secret ID (UUID / 32 hex chars)
│   └────────────────────────────────────────────────────────────── Version (v1)
└────────────────────────────────────────────────────────────────── Prefix (tk-)
```

**Components**:
- **Prefix**: `tk-` for identification and secret scanning tools
- **Version**: `v1` enables future format changes without breaking existing keys
- **HMAC Secret ID**: UUID (32 hex chars) identifying which HMAC secret validates this key
- **Random Data**: 256-bit random hex string (64 chars) providing entropy

**Benefits**:
- O(1) secret lookup (no trial-and-error across multiple secrets)
- Multiple active secrets supported simultaneously
- Clean migration path during rotation (old keys continue working)
- Stateless verification (key contains all needed information)
- No ambiguity about which secret to use

### 2. HMAC Secret Bootstrapping

Two deployment patterns supported:

**Production Pattern (Environment Variables)**:
- Check `TK_HMAC_SECRET` for single secret (no rotation)
- Check `TK_HMAC_SECRET_1`, `TK_HMAC_SECRET_2`, etc. for rotated secrets
- Multiple numbered variables enable graceful rotation:
  - Deploy with `TK_HMAC_SECRET_1` (old) + `TK_HMAC_SECRET_2` (new)
  - Generate new API keys using `TK_HMAC_SECRET_2`
  - Roll out new keys to sensors
  - Remove `TK_HMAC_SECRET_1` once all sensors migrated
- Secrets stored in external systems (Kubernetes secrets, AWS Secrets Manager, etc.)
- Operators control secret lifecycle outside application

**Development Pattern (Auto-Generation)**:
- If no `TK_HMAC_SECRET*` variables set: auto-generate 256-bit secret on first boot
- Store generated secret in database (`hmac_secrets` table)
- Load from database on subsequent restarts
- Enables immediate startup for evaluation/testing

**Rationale**: Delegating secrets management to external systems follows production best practices (SOC2 compliance requirement), while auto-generation provides zero-configuration developer experience.

### 3. HMAC Secret Storage

**Database Schema**:
```sql
CREATE TABLE hmac_secrets (
  secret_id UUID PRIMARY KEY,
  secret_hash BLOB NOT NULL,       -- SHA256 hash of the raw secret
  source TEXT NOT NULL,             -- "environment" or "auto-generated"
  created_at TIMESTAMP NOT NULL
);
```

**Loading Strategy**:
1. On startup, parse all `TK_HMAC_SECRET*` environment variables
2. For each environment secret:
   - Hash with SHA256
   - Upsert into `hmac_secrets` with `source="environment"`
3. If no environment secrets found:
   - Check database for auto-generated secret
   - If none exists: generate new 256-bit secret, store with `source="auto-generated"`
4. Build in-memory lookup map: `secret_id → secret_hash`

**Rationale**:
- Hashing secrets before database storage protects against database compromise
- SHA256 (not bcrypt) chosen for performance (verification happens on every API call)
- Storing secret ID enables efficient O(1) lookup during authentication

### 4. API Key Management

**Key Lifecycle**:
- **Provisioning**: Web UI only (CLI/automatic out of scope for MVP)
- **Naming**: Users assign human-readable names to keys for identification
- **Creation**:
  - Generate random 256-bit data
  - Select HMAC secret (defaults to most recent)
  - Compute HMAC-SHA256 hash
  - Store hash in database with UUIDv7 identifier
  - Return full key to user **once** (unrecoverable if lost)
- **Storage**: Only HMAC-SHA256 hash stored, not plaintext key
- **Rotation**: Manual through Web UI
  - Users create new key, deploy it to sensors, then revoke old key
  - Allow 2-3 active keys per tenant for controlled overlap
  - No automatic deprecation period in MVP
- **Revocation**: Mark key as revoked in database (soft delete)
- **Performance Optimization**: `last_used_at` updated periodically (max once per minute) to reduce write overhead

**Database Schema**:
```sql
CREATE TABLE api_keys (
  api_key_id UUID PRIMARY KEY,      -- UUIDv7 identifier
  tenant_id UUID NOT NULL,
  name TEXT NOT NULL,               -- User-assigned name
  key_hash BLOB NOT NULL,           -- HMAC-SHA256 hash of full key
  secret_id UUID NOT NULL,          -- FK to hmac_secrets
  created_at TIMESTAMP NOT NULL,
  last_used_at TIMESTAMP,
  revoked_at TIMESTAMP,
  FOREIGN KEY (secret_id) REFERENCES hmac_secrets(secret_id)
);
```

**Scope**: Single-tenant in MVP, full sensor permissions (read rules + write events). No granular permissions.

### 5. Authentication Flow

**Request Authentication** (gRPC metadata `x-api-key`):
1. Extract API key from `x-api-key` metadata
2. Parse format: `tk-v1-<secret-id>-<random-data>`
3. Lookup HMAC secret using `secret-id` (O(1) from in-memory map)
4. Compute HMAC-SHA256 hash of full key
5. Compare against stored hash in `api_keys` table
6. Check revocation status
7. Update `last_used_at` if >1 minute since last update
8. Proceed with request or return error

**Error Responses** (gRPC status codes):
- Missing API key: `UNAUTHENTICATED` - "API key required in x-api-key metadata"
- Invalid format: `UNAUTHENTICATED` - "Invalid API key format"
- Unknown key: `UNAUTHENTICATED` - "Invalid API key"
- Revoked key: `PERMISSION_DENIED` - "API key has been revoked"

**Security Monitoring**:
- Log all authentication failures with client IP address
- Enable detection of brute-force attempts or compromised keys

### 6. HMAC vs Bcrypt for API Keys

**Decision**: Use HMAC-SHA256, not bcrypt

**Rationale**:
- **Performance**: Authentication happens on every gRPC call (potentially thousands per second per sensor)
- **Sufficient security**: 256-bit random data provides strong entropy, HMAC-SHA256 is cryptographically secure
- **Different threat model**: API keys shown once and stored by clients (not memorized like passwords)
- **Bcrypt designed for passwords**: Intentionally slow to resist brute-force against weak passwords, unnecessary overhead for high-entropy API keys

## Consequences

### Benefits

1. **Developer Experience**: Zero-configuration startup enables immediate evaluation and testing
2. **Production Security**: Environment variables delegate secrets management to infrastructure (Kubernetes, Vault, etc.)
3. **Efficient Rotation**: Multiple active secrets enable graceful key migration without downtime
4. **O(1) Performance**: Secret ID embedded in key format enables direct lookup, no trial-and-error
5. **Audit Trail**: `last_used_at` tracking helps identify unused keys for cleanup
6. **Clear Security Boundaries**: HMAC secrets managed externally, API keys managed in application
7. **SOC2 Compliance**: Externalized secrets management aligns with compliance requirements

### Tradeoffs

1. **Single Recovery Point**: Keys shown once at creation, unrecoverable if lost (must revoke and regenerate)
2. **Manual Rotation**: No automatic key expiration or deprecation warnings in MVP
3. **No Granular Permissions**: All keys have full sensor permissions (read rules + write events)
4. **Secret Storage Risk**: Auto-generated secrets stored in database (mitigated by hashing)
5. **Performance Trade-off**: `last_used_at` updates create write load (mitigated by 1-minute throttle)
6. **Race Conditions**: Multiple environment variable secrets loaded on startup may have timing issues (non-concern for MVP single-instance deployment)

### Operational Implications

1. **Secret Distribution**: Operators must securely distribute environment variables or auto-generated secrets to sensors
2. **Key Rotation Process**:
   - Add new environment variable (`TK_HMAC_SECRET_2`)
   - Restart service to load new secret
   - Generate new API keys in Web UI
   - Deploy new keys to sensors
   - Remove old environment variable and revoke old keys
3. **Monitoring**: Track authentication failure rates to detect compromised keys
4. **Database Backup**: Database contains hashed secrets (auto-generated pattern), must be secured
5. **NTP Synchronization**: UUIDv7 identifiers require time sync across instances (document requirement)

## Implementation

1. **Database Schema**:
   - Create `hmac_secrets` table for secret storage
   - Create `api_keys` table with HMAC hash storage
   - Add indexes on `api_key_id`, `tenant_id`, `secret_id`

2. **HMAC Secret Loading**:
   - Parse `TK_HMAC_SECRET*` environment variables on startup
   - Hash and upsert into `hmac_secrets` table
   - Fall back to auto-generation if no environment secrets
   - Build in-memory lookup map for O(1) access

3. **API Key Generation** (Web UI):
   - Generate 256-bit random data using `crypto/rand`
   - Select HMAC secret (most recent by default)
   - Compute HMAC-SHA256 hash
   - Store in database with UUIDv7 identifier
   - Return formatted key to user once

4. **Authentication Middleware** (gRPC interceptor):
   - Extract `x-api-key` from metadata
   - Parse format and lookup secret
   - Verify HMAC-SHA256 hash
   - Check revocation status
   - Update `last_used_at` (throttled to 1-minute intervals)
   - Return appropriate error codes

5. **Security Logging**:
   - Log authentication failures with client IP
   - Exclude sensitive data (no key values in logs)
   - Enable monitoring for attack detection

## Future Considerations

- **Automatic key expiration**: Add `expires_at` field with UI warnings before expiration
- **Granular permissions**: Separate read-only keys from write-only keys
- **Audit logging**: Track all key usage for compliance requirements
- **Multiple tenants**: Extend authentication to support multi-tenant deployments
- **CLI provisioning**: Command-line tools for API key generation and management
- **Key recovery**: Secure key escrow or backup mechanisms for lost keys
