---
doc_type: spoke
status: active
date_created: 2025-01-15
date_updated: 2025-01-20
primary_category: security
hub_document: doc/06-security/README.md
tags:
  - authentication
  - jwt
  - security
maintainer: Security Team
---

# JWT Token Authentication

## Context

This spoke documents the detailed implementation of JWT token-based authentication used throughout the system. Token-based authentication provides stateless verification without requiring session storage or database lookups on every request.

Detailed implementation documentation is needed separate from the hub because authentication involves complex token structure, multiple validation steps, error handling patterns, and performance considerations that would overwhelm a strategic overview.

**Hub Document**: This document is part of the Security Architecture hub. See doc/06-security/README.md for strategic overview and relationships to authorization, encryption, and threat mitigation.

## Token Structure

JWT tokens use three-part structure: header, payload, and signature. All tokens signed using RS256 (RSA with SHA-256) for asymmetric verification allowing distributed validation without sharing secrets.

### Header

Standard JWT header specifying token type and signing algorithm.

```json
{
  "alg": "RS256",
  "typ": "JWT"
}
```

### Payload

Token payload contains claims identifying user and authorization scope.

```json
{
  "sub": "user-uuid-here",
  "name": "User Name",
  "email": "user@example.com",
  "roles": ["user", "admin"],
  "permissions": ["rules:read", "rules:write", "events:read"],
  "iat": 1705334400,
  "exp": 1705420800,
  "iss": "trapperkeeper-auth",
  "aud": "trapperkeeper-api"
}
```

**Key Claims**:

- `sub`: User UUID (subject identifier)
- `roles`: User role assignments for coarse-grained authorization
- `permissions`: Specific permissions for fine-grained authorization
- `iat`: Issued at timestamp (Unix epoch)
- `exp`: Expiration timestamp (Unix epoch)
- `iss`: Issuer identifier
- `aud`: Intended audience

### Signature

RSA signature computed over header and payload using private key. Signature verified using public key distributed to all services.

**Example**:

```go
// Token generation (auth service only)
claims := Claims{
    Sub:         user.ID,
    Name:        user.Name,
    Email:       user.Email,
    Roles:       user.Roles,
    Permissions: user.Permissions,
    Iat:         time.Now().Unix(),
    Exp:         time.Now().Add(24 * time.Hour).Unix(),
    Iss:         "trapperkeeper-auth",
    Aud:         "trapperkeeper-api",
}

token, err := jwt.EncodeToken(&header, &claims, privateKey)
if err != nil {
    return "", err
}
```

## Token Validation

All services validate JWT tokens on every authenticated request. Validation includes multiple checks performed in sequence.

### Validation Steps

1. **Signature verification**: Verify RSA signature using public key
2. **Expiration check**: Ensure current time before `exp` claim
3. **Issuer verification**: Verify `iss` matches expected issuer
4. **Audience verification**: Verify `aud` includes this service
5. **Claims validation**: Ensure required claims present and well-formed

**Example**:

```go
func ValidateToken(token string, publicKey *rsa.PublicKey) (*Claims, error) {
    // Decode and verify signature
    tokenData, err := jwt.ParseWithClaims(token, &Claims{}, func(t *jwt.Token) (interface{}, error) {
        if _, ok := t.Method.(*jwt.SigningMethodRSA); !ok {
            return nil, fmt.Errorf("unexpected signing method: %v", t.Header["alg"])
        }
        return publicKey, nil
    })
    if err != nil {
        return nil, fmt.Errorf("token decode failed: %w", err)
    }

    claims, ok := tokenData.Claims.(*Claims)
    if !ok || !tokenData.Valid {
        return nil, errors.New("invalid token claims")
    }

    // Verify expiration
    now := time.Now().Unix()
    if claims.Exp <= now {
        return nil, ErrTokenExpired
    }

    // Verify issuer
    if claims.Iss != "trapperkeeper-auth" {
        return nil, ErrInvalidIssuer
    }

    // Verify audience
    if claims.Aud != "trapperkeeper-api" {
        return nil, ErrInvalidAudience
    }

    return claims, nil
}
```

**Error Handling**: Validation failures return specific error types enabling appropriate HTTP status codes (401 for invalid/expired tokens, 403 for insufficient permissions).

**Cross-References**:

- doc/06-security/authorization.md: How claims feed authorization decisions
- doc/08-resilience/error-handling.md: Authentication error handling patterns
- error-handling-index.md: Standard error response format

## Token Lifecycle

Tokens have 24-hour lifetime balancing security and user experience. Shorter lifetime increases security but requires frequent re-authentication; longer lifetime improves UX but increases exposure risk.

### Token Issuance

Tokens issued after successful authentication (username/password or multi-factor). Auth service generates token, logs issuance event, and returns token to client.

**Example**:

```go
func Authenticate(ctx context.Context, username, password string, mfaCode *string) (*TokenResponse, error) {
    // Verify password
    user, err := verifyPassword(ctx, username, password)
    if err != nil {
        return nil, err
    }

    // Verify MFA if enabled
    if user.MFAEnabled {
        if mfaCode == nil {
            return nil, ErrMFARequired
        }
        if err := verifyMFACode(user, *mfaCode); err != nil {
            return nil, err
        }
    }

    // Generate token
    token, err := generateToken(user)
    if err != nil {
        return nil, err
    }

    // Log issuance
    logAuthEvent(AuthEvent{
        Type:      "TokenIssued",
        UserID:    user.ID,
        Timestamp: time.Now(),
        IP:        requestIP(ctx),
    })

    return &TokenResponse{
        Token:     token,
        ExpiresAt: time.Now().Add(24 * time.Hour),
        TokenType: "Bearer",
    }, nil
}
```

### Token Rotation

Tokens automatically expire after 24 hours requiring re-authentication. No token refresh mechanism provided - users must re-authenticate to obtain new token.

Design decision: No refresh tokens simplifies implementation and reduces security risk (refresh tokens become high-value targets). 24-hour lifetime provides reasonable UX without refresh complexity.

**Cross-References**:

- doc/06-security/README.md: Token rotation strategy discussion
- ../06-security/authentication-web-ui.md: Authentication design rationale and user management

### Token Revocation

Token revocation not implemented - tokens valid until expiration. Revocation would require maintaining revocation list defeating stateless architecture benefits.

If immediate revocation required (compromised account), user password changed invalidating future token issuance. Existing tokens remain valid until expiration (max 24 hours).

**Known Limitation**: No immediate token revocation - see Edge Cases section.

## Performance Characteristics

Token validation optimized for minimal latency impact.

### Validation Latency

RSA signature verification: ~100μs on modern CPUs. Public key cached in memory avoiding disk I/O. Claims validation (string comparisons, timestamp checks): ~1μs.

Total validation overhead: ~100μs per request acceptable for our performance targets (<50ms p99 latency).

**Cross-References**:

- performance-index.md: Authentication performance targets
- doc/05-performance/README.md: Overall performance model

### Caching

Public keys cached in memory with 1-hour TTL. Key rotation triggers cache invalidation across all services via pub/sub message.

Claims not cached - validation performed on every request ensuring consistent authorization even if permissions changed. Performance impact minimal due to fast validation.

**Example**:

```go
// Public key cache with TTL
var (
    publicKeyCache struct {
        sync.RWMutex
        key      *rsa.PublicKey
        cachedAt time.Time
    }
)

func GetPublicKey() (*rsa.PublicKey, error) {
    publicKeyCache.RLock()
    if publicKeyCache.key != nil && time.Since(publicKeyCache.cachedAt) < time.Hour {
        key := publicKeyCache.key
        publicKeyCache.RUnlock()
        return key, nil
    }
    publicKeyCache.RUnlock()

    // Cache miss or expired - fetch new key
    publicKeyCache.Lock()
    defer publicKeyCache.Unlock()

    // Double-check after acquiring write lock
    if publicKeyCache.key != nil && time.Since(publicKeyCache.cachedAt) < time.Hour {
        return publicKeyCache.key, nil
    }

    key, err := fetchPublicKeyFromAuthService()
    if err != nil {
        return nil, err
    }

    publicKeyCache.key = key
    publicKeyCache.cachedAt = time.Now()

    return key, nil
}
```

## Edge Cases and Limitations

**Known Limitations**:

- **No token revocation**: Tokens valid until expiration even if user account compromised. Mitigation: 24-hour token lifetime limits exposure window. Password change prevents new token issuance.
- **No offline validation**: Signature verification requires public key. If auth service unavailable and key cache expired, validation fails. Mitigation: 1-hour key cache TTL provides resilience window.
- **No token refresh**: Users must re-authenticate after 24 hours. Some users find this disruptive. Mitigation: 24-hour lifetime balances security and UX.

**Edge Cases**:

- **Clock skew**: System clocks out of sync cause premature expiration or delayed expiration. Mitigation: 5-minute clock skew tolerance in validation.
- **Token in URL**: Tokens passed in URL query params logged by proxies/servers. Expected behavior: Reject authentication via URL params - require Authorization header.
- **Multiple concurrent requests**: Token nearing expiration may succeed for some requests and fail for others racing against expiration. Expected behavior: Client should refresh token proactively before expiration.

## Related Documents

**Dependencies** (read these first):

- ../06-security/authentication-web-ui.md: Authentication design decisions and rationale
- ../06-security/authentication-sensor-api.md: API authentication requirements

**Related Spokes** (siblings in this hub):

- doc/06-security/authorization.md: Complements authentication with authorization using token claims
- doc/06-security/threat-mitigation.md: Rate limiting protects authentication endpoints
- doc/06-security/tls-certificates.md: TLS secures token transmission

**Extended by** (documents building on this):

- web-ui-authentication.md: Web UI authentication flows using JWT tokens
- api-client-authentication.md: API client authentication patterns
