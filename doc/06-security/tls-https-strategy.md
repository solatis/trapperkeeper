---
doc_type: spoke
status: active
primary_category: security
hub_document: doc/06-security/README.md
tags:
  - tls
  - https
  - grpc
  - certificates
  - transport-security
---

# TLS/HTTPS Strategy

## Context

TrapperKeeper has two network-facing services requiring transport security: tk-web-ui (HTTP/HTTPS on port 8080) and tk-sensor-api (gRPC on port 50051). This document specifies flexible TLS strategies supporting direct termination, reverse proxy, and development scenarios.

**Hub Document**: This document is part of the [Security Architecture](README.md). See [Security Hub](README.md) Section 3 for strategic overview of transport security and defense-in-depth principles (TLS protects transport, authentication protects access).

## HTTP/HTTPS (Web UI - Port 8080)

### Deployment Modes

**HTTPS Mode** (Direct TLS Termination):

- TLS 1.3 minimum with modern cipher suites
- Cipher suites: TLS_AES_256_GCM_SHA384, TLS_AES_128_GCM_SHA256, TLS_CHACHA20_POLY1305_SHA256
- Secure cookie flag always enabled
- Certificate management via filesystem (user-provided certificates)
- CLI flags: `--tls-cert-file <path>` and `--tls-key-file <path>`

**HTTP Mode with Reverse Proxy**:

- Server runs in HTTP, reverse proxy (nginx, Caddy, AWS ALB) terminates TLS
- Secure cookie flag automatically enabled when `X-Forwarded-Proto: https` header detected
- Custom middleware inspects `X-Forwarded-Proto` header per-request
- Recommended deployment pattern for production

**HTTP Development Mode**:

- Local development without TLS
- Secure cookie flag disabled
- Localhost-only binding recommended
- Clear warning logged on startup: "Running in HTTP mode - secure cookies disabled"

### X-Forwarded-Proto Middleware

Custom middleware detects HTTPS via reverse proxy headers:

```go
import (
    "context"
    "net/http"
    "strings"
)

// secureConnectionKey is the context key for HTTPS detection
type contextKey string
const secureConnectionKey contextKey = "secure_connection"

// DetectHTTPSMiddleware detects HTTPS via X-Forwarded-Proto header
func DetectHTTPSMiddleware(next http.Handler) http.Handler {
    return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
        // Check X-Forwarded-Proto header
        proto := r.Header.Get("X-Forwarded-Proto")
        isHTTPS := strings.EqualFold(proto, "https")

        // Store HTTPS detection in request context for scs
        ctx := context.WithValue(r.Context(), secureConnectionKey, isHTTPS)
        r = r.WithContext(ctx)

        next.ServeHTTP(w, r)
    })
}

// IsSecureConnection retrieves HTTPS status from context
func IsSecureConnection(ctx context.Context) bool {
    if val, ok := ctx.Value(secureConnectionKey).(bool); ok {
        return val
    }
    return false
}
```

**Integration with scs**:

- Middleware runs before scs session management in middleware chain
- Session cookie secure flag modified based on `X-Forwarded-Proto` header
- SameSite=Lax and httpOnly=true always enabled regardless of mode

### Certificate Management (HTTP)

**Certificate Format**:

- PEM-encoded X.509 certificates
- Private key format: PEM-encoded RSA or ECDSA keys

**Storage Paths** (example):

- Certificate: `/etc/trapperkeeper/tls/web-ui-cert.pem`
- Private key: `/etc/trapperkeeper/tls/web-ui-key.pem`

**Certificate Validation** (startup):

- Validate certificate/key pair matching
- Check certificate expiry (warn if <30 days)
- Fail startup if certificates expired or mismatched
- Error messages follow Validation Hub error format with structured error codes

**Rotation**:

- Manual process requiring service restart
- Monitor expiry with 30-day warning threshold
- No automatic certificate provisioning (Let's Encrypt integration out of scope for MVP)

### Reverse Proxy Configuration Examples

**Nginx Configuration**:

```nginx
# TLS termination at nginx, proxying to tk-web-ui on localhost:8080
server {
    listen 443 ssl http2;
    server_name trapperkeeper.example.com;

    # TLS configuration
    ssl_certificate /etc/nginx/ssl/trapperkeeper.crt;
    ssl_certificate_key /etc/nginx/ssl/trapperkeeper.key;
    ssl_protocols TLSv1.3;
    ssl_ciphers 'TLS_AES_256_GCM_SHA384:TLS_AES_128_GCM_SHA256:TLS_CHACHA20_POLY1305_SHA256';
    ssl_prefer_server_ciphers on;

    # Proxy configuration
    location / {
        proxy_pass http://localhost:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;

        # CRITICAL: This header enables secure cookie detection
        proxy_set_header X-Forwarded-Proto https;
    }
}

# Redirect HTTP to HTTPS
server {
    listen 80;
    server_name trapperkeeper.example.com;
    return 301 https://$server_name$request_uri;
}
```

**Caddy Configuration**:

```caddy
# Caddy automatically provisions Let's Encrypt certificates
trapperkeeper.example.com {
    # TLS configuration (Caddy defaults to TLS 1.3 with modern ciphers)
    tls /etc/caddy/trapperkeeper.crt /etc/caddy/trapperkeeper.key

    # Reverse proxy to tk-web-ui
    reverse_proxy localhost:8080 {
        # Caddy automatically sets X-Forwarded-Proto header
        header_up X-Forwarded-Proto https
    }
}
```

## gRPC/TLS (Sensor API - Port 50051)

### Deployment Modes

**TLS Mode** (Direct TLS Termination - Production):

- TLS 1.3 minimum (same cipher suites as HTTP)
- HMAC authentication ([Authentication (Sensor API)](authentication-sensor-api.md)) provides authentication layer
- Defense in depth: TLS secures transport, HMAC authenticates requests
- Certificate management via filesystem (user-provided certificates)
- CLI flags: `--grpc-tls-cert-file <path>` and `--grpc-tls-key-file <path>`

**Plaintext Mode** (Development Only):

- gRPC without TLS
- WARNING: Never deploy to production without TLS
- HMAC authentication still required (authentication independent of transport security)
- Clear warning logged on startup: "Running gRPC in plaintext mode - TLS disabled"

### Certificate Management (gRPC)

**Certificate Format**:

- PEM-encoded X.509 certificates
- Private key format: PEM-encoded RSA or ECDSA keys
- Separate certificate from Web UI (different service, different port)

**Storage Paths** (example):

- Certificate: `/etc/trapperkeeper/tls/sensor-api-cert.pem`
- Private key: `/etc/trapperkeeper/tls/sensor-api-key.pem`

**Certificate Validation** (startup):

- Validate certificate/key pair matching
- Check certificate expiry (warn if <30 days)
- Fail startup if certificates expired or mismatched
- Use crypto/tls and google.golang.org/grpc/credentials for TLS configuration

**Rotation**:

- Manual renewal process requires service restart
- Monitor expiry for BOTH HTTP and gRPC certificates (independent schedules)
- Independent renewal allows different expiry dates per service

### mTLS Evaluation

**Decision**: NOT implemented for MVP.

**Rationale**:

- HMAC-SHA256 API keys ([Authentication (Sensor API)](authentication-sensor-api.md)) already provide authentication
- Defense in depth: TLS provides transport encryption, HMAC provides request authentication
- mTLS adds significant operational complexity (client certificate management, CA infrastructure)

**Future Consideration**: mTLS could replace HMAC if client certificate management becomes tractable (high-security deployments with hardware security modules).

## Certificate Generation

### Development: Self-Signed Certificates

**Option 1: Using OpenSSL**:

```bash
# Web UI certificate (HTTP/HTTPS)
openssl req -x509 -newkey rsa:4096 -nodes \
  -keyout web-ui-key.pem \
  -out web-ui-cert.pem \
  -days 365 \
  -subj "/CN=localhost" \
  -addext "subjectAltName=DNS:localhost,IP:127.0.0.1"

# Sensor API certificate (gRPC)
openssl req -x509 -newkey rsa:4096 -nodes \
  -keyout sensor-api-key.pem \
  -out sensor-api-cert.pem \
  -days 365 \
  -subj "/CN=localhost" \
  -addext "subjectAltName=DNS:localhost,IP:127.0.0.1"
```

**Option 2: Using mkcert** (Automatic Local CA):

```bash
# Install mkcert (macOS)
brew install mkcert
mkcert -install

# Generate certificates for both services
mkcert -cert-file web-ui-cert.pem -key-file web-ui-key.pem localhost 127.0.0.1
mkcert -cert-file sensor-api-cert.pem -key-file sensor-api-key.pem localhost 127.0.0.1
```

### Production: Certificate Authority

**Let's Encrypt (Recommended)** Use reverse proxy (nginx/Caddy) for automatic certificate management:

```bash
# Certbot with nginx
certbot --nginx -d trapperkeeper.example.com

# For gRPC (separate domain/certificate)
certbot certonly --standalone -d grpc.trapperkeeper.example.com

# Link to expected paths
ln -s /etc/letsencrypt/live/grpc.trapperkeeper.example.com/fullchain.pem \
      /etc/trapperkeeper/tls/sensor-api-cert.pem
ln -s /etc/letsencrypt/live/grpc.trapperkeeper.example.com/privkey.pem \
      /etc/trapperkeeper/tls/sensor-api-key.pem
```

**Internal CA or Purchased Certificates**:

1. Generate private key
2. Create Certificate Signing Request (CSR)
3. Submit CSR to CA
4. Download signed certificate and intermediate chain
5. Concatenate certificate + intermediate chain into single PEM file
6. Configure services with certificate paths

## Certificate Verification

Verify certificates before deployment:

```bash
# Check certificate expiry
openssl x509 -in web-ui-cert.pem -noout -enddate
openssl x509 -in sensor-api-cert.pem -noout -enddate

# Verify certificate/key pair matching
openssl x509 -in web-ui-cert.pem -noout -modulus | openssl md5
openssl rsa -in web-ui-key.pem -noout -modulus | openssl md5
# (hashes must match)

# Test HTTPS connection (Web UI)
curl -v https://localhost:8080/health

# Test gRPC TLS connection (Sensor API)
grpcurl -v -d '{"tags": ["test"]}' \
  localhost:50051 \
  trapperkeeper.sensor.v1.SensorAPI/SyncRules
```

## Edge Cases and Limitations

**Known Limitations**:

- Two certificates to manage: Separate HTTP and gRPC certificates require independent monitoring and renewal
- Manual certificate management: No auto-renewal, operators provision certificates manually
- Header spoofing risk: Misconfigured reverse proxy could allow attacker to spoof `X-Forwarded-Proto` header (set secure flag incorrectly)
- No mTLS: Sensor API uses HMAC instead of mutual TLS (simpler but different trust model)
- Custom middleware required: Dynamic secure flag modification based on X-Forwarded-Proto requires custom middleware

**Edge Cases**:

- Certificate expiry during operation: Service continues running with expired certificate (clients reject TLS handshake)
- Mismatched certificate/key: Startup validation catches mismatch, prevents service start with clear error
- X-Forwarded-Proto missing: Reverse proxy mode without header sets secure flag to false (potential security issue)
- Certificate rotation: Requires service restart (brief downtime during certificate renewal)

## Related Documents

**Dependencies** (read these first):

- [API Service Architecture](../02-architecture/api-service.md): gRPC TLS termination requirements
- [Web Framework](../09-operations/web-framework.md): HTTP middleware system for X-Forwarded-Proto detection
- [Configuration Management](../09-operations/configuration.md): Certificate path configuration, startup validation
- [Validation Hub](../07-validation/README.md): TLS certificate validation and structured error messages

**Related Spokes** (siblings in this hub):

- [Authentication (Web UI)](authentication-web-ui.md): Cookie secure flag coordination with TLS modes
- [Encryption Strategy](encryption.md): TLS transport encryption (Section 5), TLS private key management (Section 4.4)

**Extended by**:

- [Service Architecture](../02-architecture/README.md): Defines TLS termination strategy for tk-web-ui and tk-sensor-api on separate ports
