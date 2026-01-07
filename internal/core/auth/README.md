# Authentication Package

## Overview

HMAC-based API key authentication for gRPC sensor-api and future HTTP services. Provides O(1) secret lookup via secret_id embedded in API key format, constant-time HMAC-SHA256 validation, and gRPC interceptor chain integration. Separated from api/ handlers to enable independent testing and reuse across multiple service types.

## Architecture

```
Request -> gRPC Interceptor (extract x-api-key from metadata)
       -> ParseAPIKey (extract secret_id from tk-v1-<secret_id>-<random>)
       -> O(1) secret lookup in map[secret_id][]byte
       -> ComputeHMAC(secret, apiKey)
       -> Query database by key_hash (unique constraint)
       -> Check revoked_at status
       -> Update last_used_at (1-minute throttle)
       -> Inject tenant_id into context
       -> Handler invoked with authenticated context
```

Auth interceptor executes before handler logic. Handlers receive context with tenant_id already validated.

## Design Decisions

**HMAC-SHA256 over bcrypt for API keys**: API keys authenticated on every gRPC call (potentially thousands/second). Bcrypt's intentional slowness (100ms+) designed for password brute-force resistance is unnecessary for 256-bit entropy keys. HMAC-SHA256 provides cryptographic security with O(1) verification and sub-millisecond performance.

**O(1) secret lookup via secret_id in key format**: Multiple HMAC secrets supported for rotation. Trial-and-error validation would be O(n) per request. Embedding secret_id in key format (`tk-v1-<secret_id>-<random>`) enables direct map lookup without iterating all secrets.

**5-tier error taxonomy**: Distinguishing error types enables targeted debugging and monitoring. ErrMissingKey vs ErrInvalidKeyFormat vs ErrUnknownKey vs ErrInvalidKey vs ErrKeyRevoked map to distinct gRPC codes. UNAUTHENTICATED for missing/invalid (doesn't confirm key existence), PERMISSION_DENIED for revoked (confirms key exists but blocked). Operators differentiate misconfiguration from security events in logs.

**1-minute throttle on last_used_at updates**: Updating timestamp on every request creates write amplification (potentially thousands of writes/second per active sensor). 1-minute window reduces writes by 99%+ for active sensors while maintaining acceptable precision for audit purposes. Do not rely on last_used_at for sub-minute accuracy.

**Separate auth/ package from api/**: Auth interceptor used by both gRPC (Phase 4) and future HTTP services (Phase 5). Shared package avoids duplication, enables isolated testing of auth logic, and allows web-ui to reuse without importing gRPC handler dependencies.

**102-character API key format**: tk-v1 prefix (5) + separators (2) + UUID secret_id (32 hex, no hyphens) + random_data (64 hex = 256 bits entropy). Total 102 chars fits single terminal line, VARCHAR(128) with headroom. Exceeds NIST 256-bit recommendation.

## Invariants

1. **Auth happens before any handler logic**: gRPC interceptor chain ensures authentication completes before handler invoked. Handler code can assume valid tenant context. Interceptor returns error before calling handler() if authentication fails.

2. **HMAC secrets loaded at startup**: Secret map built once during initialization from environment variables, not per-request. Secret rotation requires service restart or explicit reload signal. Secrets never read from config files (security requirement).

3. **Revoked keys return PERMISSION_DENIED, not UNAUTHENTICATED**: Distinguishes "key exists but revoked" from "key unknown". Important for debugging and audit. Unknown or invalid keys return UNAUTHENTICATED without confirming existence.

4. **last_used_at throttled to 1-minute precision**: Do not rely on last_used_at for sub-minute accuracy. Acceptable for audit logging, unsuitable for rate limiting or precise activity tracking. Updates are best-effort (ignores failures).

5. **Constant-time HMAC comparison**: hmac.Equal() prevents timing attacks. Never use bytes.Equal() or string comparison for HMAC validation.

## Tradeoffs

| Choice                  | Benefit                                  | Cost                                                   |
| ----------------------- | ---------------------------------------- | ------------------------------------------------------ |
| HMAC over bcrypt        | Sub-millisecond auth, horizontal scaling | Must ensure high-entropy keys (256-bit minimum)        |
| 1-minute throttle       | Reduced write amplification              | Imprecise usage tracking, unsuitable for rate limiting |
| 5-tier error taxonomy   | Targeted debugging, security monitoring  | More complex error handling in clients                 |
| Secret_id in key format | O(1) lookup, rotation support            | Slightly longer API keys (102 vs ~88 chars)            |
