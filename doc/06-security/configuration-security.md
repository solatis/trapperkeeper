---
doc_type: spoke
status: active
date_created: 2025-11-07
primary_category: security
hub_document: /Users/lmergen/git/trapperkeeper/doc/06-security/README.md
tags:
  - configuration
  - secrets-management
  - environment-variables
  - security-policy
---

# Configuration Security

## Context

TrapperKeeper configuration includes sensitive values (HMAC secrets, database passwords, API keys) that must never be stored in configuration files to prevent accidental exposure through version control, log files, or overly permissive file permissions. This document specifies security policies for configuration management.

**Hub Document**: This document is part of the [Security Architecture](README.md). See [Security Hub](README.md) Section 5 for strategic overview of secret management and configuration security constraints.

## Secrets Rejection Policy

### Configuration Sources and Security

TrapperKeeper uses three-tier configuration precedence: file < env < CLI (CLI arguments override environment variables override config files).

**Security Constraint**: Secrets MUST be provided via environment variables or CLI arguments, NEVER configuration files.

**Rationale**:

- Configuration files often committed to version control (accidental secret exposure)
- Configuration files may have overly permissive file permissions (644 vs 600)
- Environment variables and CLI arguments provide ephemeral secret storage
- Infrastructure-as-code (Kubernetes, Docker Compose) manages secrets securely via env vars

### Secret Value Validation

Services validate at startup that configuration files do NOT contain secrets:

**Validation Rules**:

1. Parse configuration file (TOML format)
2. Check for forbidden secret fields:
   - `sensor_api.hmac_secret` → REJECTED
   - `database.password` → REJECTED
   - Any field ending in `_secret`, `_key`, `_password`, `_token` → REJECTED
3. If secrets found: Fail startup with error message
4. If no secrets found: Continue with service initialization

**Validation Enforcement**:

```go
// Pseudo-code for security validation
func validateNoSecretsInFile(config *Config, source ConfigSource) error {
    if source == ConfigSourceFile {
        if config.SensorAPI.HMACSecret != nil {
            return &ConfigError{
                Field:       "sensor_api.hmac_secret",
                Message:     "HMAC secrets cannot be stored in configuration files",
                Remediation: "Use TK_HMAC_SECRET environment variable or --hmac-secret CLI flag",
            }
        }
        if config.Database.Password != nil {
            return &ConfigError{
                Field:       "database.password",
                Message:     "Database passwords cannot be stored in configuration files",
                Remediation: "Use TK_DATABASE_PASSWORD environment variable or --db-password CLI flag",
            }
        }
    }
    return nil
}
```

**Error Message Format** (follows Validation Hub error format):

```
Configuration validation failed:
  - sensor_api.hmac_secret: Secret values cannot be stored in configuration files
    → Use TK_HMAC_SECRET environment variable or --hmac-secret CLI flag instead
```

## Environment Variable Policy

### Required Environment Variables (Production)

**HMAC Secrets** (Sensor API authentication):

- `TK_HMAC_SECRET`: Single HMAC secret (no rotation)
- `TK_HMAC_SECRET_1`, `TK_HMAC_SECRET_2`, ...: Multiple secrets for rotation
- Format: 256-bit hex string (64 characters)
- Generation: `openssl rand -hex 32` or equivalent

**Database Credentials**:

- `TK_DATABASE_URL`: Complete database connection string (includes password if required)
- Format: `postgresql://user:password@host/database` or `sqlite:///path/to/db.sqlite`

**TLS Private Key Passphrases** (if using encrypted keys):

- `TK_TLS_KEY_PASSPHRASE`: Passphrase for Web UI TLS private key
- `TK_GRPC_TLS_KEY_PASSPHRASE`: Passphrase for Sensor API gRPC TLS private key

### Environment Variable Naming Convention

**Format**: `TK_<SECTION>_<KEY>` (all uppercase, underscores separate words).

**Examples**:

- `TK_SENSOR_API_PORT` → `[sensor_api] port`
- `TK_WEB_UI_HOST` → `[web_ui] host`
- `TK_DATABASE_MAX_CONNECTIONS` → `[database] max_connections`
- `TK_HMAC_SECRET` → Special case (not in config file schema)

**Type Coercion**:

- Boolean: `true`, `false`, `1`, `0`, `yes`, `no` (case-insensitive)
- Integer: Numeric strings, reject non-numeric values
- Duration: Suffixes supported (`ms`, `s`, `m`, `h` - e.g., `5000ms`, `5s`)
- Arrays: Comma-separated values (e.g., `TK_ALLOWED_ORIGINS=http://localhost:3000,http://localhost:8080`)

## CLI Argument Policy

### Security-Sensitive Flags

**HMAC Secret**:

```bash
trapperkeeper sensor-api --hmac-secret <SECRET>
```

- Accepts 256-bit hex string (64 characters)
- Overrides `TK_HMAC_SECRET` environment variable
- Useful for testing (avoid for production - use env vars)

**Database URL**:

```bash
trapperkeeper sensor-api --db-url "postgresql://user:password@host/database"
```

- Overrides `TK_DATABASE_URL` environment variable
- Useful for quick testing

**TLS Certificate Passphrases**:

```bash
trapperkeeper web-ui --tls-key-passphrase <PASSPHRASE>
```

- Only required if TLS private key is encrypted

### Warning: CLI Arguments in Process List

CLI arguments visible in process listings (`ps aux`, `/proc/<pid>/cmdline`):

- **Avoid CLI arguments for secrets in production**
- Use environment variables instead (not visible in process list)
- CLI arguments acceptable for development/testing only

## HMAC Secret Management

### Development Mode (Auto-Generation)

If no `TK_HMAC_SECRET*` environment variables configured:

1. Generate 256-bit cryptographically secure random secret on first boot
2. Store SHA256 hash in database (`hmac_secrets` table with `source="auto-generated"`)
3. Load from database on subsequent restarts
4. Log warning: "Using auto-generated HMAC secret - for production, set TK_HMAC_SECRET environment variable"

**Security Implications**:

- Auto-generated secret stored in database (hashed with SHA256)
- Database backup required for API key validation
- Cannot migrate database without recreating API keys (secret embedded in hash)

### Production Mode (Environment Variables)

**Single Secret** (no rotation):

```bash
export TK_HMAC_SECRET="<256-bit hex string>"
trapperkeeper sensor-api
```

**Multiple Secrets** (rotation support):

```bash
export TK_HMAC_SECRET_1="<old secret>"
export TK_HMAC_SECRET_2="<new secret>"
trapperkeeper sensor-api
```

**Zero-Downtime Rotation Procedure**:

1. Set `TK_HMAC_SECRET_2` with new secret (keep `TK_HMAC_SECRET_1` active)
2. Restart service (both secrets loaded, old API keys still valid)
3. Generate new API keys via Web UI (uses `TK_HMAC_SECRET_2`)
4. Roll out new API keys to sensors (gradual migration)
5. Monitor last_used_at for old API keys (identify stragglers)
6. Remove `TK_HMAC_SECRET_1` when all sensors migrated
7. Restart service (old API keys invalidated)

**Cross-Reference**: [Authentication (Sensor API)](authentication-sensor-api.md) Section on HMAC Secret Bootstrapping for complete implementation.

## Attack Prevention

### Secrets Exposure via Configuration Files

**Attack Scenario**: Developer commits `trapperkeeper.toml` with `hmac_secret = "..."` to version control.

**Prevention**:

1. Validation at startup rejects secrets in config files
2. `.gitignore` includes `*.toml` (default recommendation)
3. Documentation emphasizes environment variables for secrets
4. Error messages provide clear remediation guidance

### Secrets Exposure via Logs

**Attack Scenario**: Application logs include secret values in error messages or debug output.

**Prevention**:

1. Never log HMAC secrets, passwords, API keys (code review enforcement)
2. Redact secret values in error messages (show `<redacted>` instead of actual value)
3. Log only secret IDs or last 4 characters for debugging (never full secret)

**Example Logging**:

```go
// Good: Log only secret ID
log.Printf("Loaded HMAC secret: id=%s", secretID)

// Bad: Log full secret
log.Printf("Loaded HMAC secret: %s", hmacSecret) // NEVER DO THIS
```

### Secrets Injection via CLI

**Attack Scenario**: Attacker injects secrets via CLI arguments visible in process list.

**Prevention**:

1. Documentation warns against CLI secrets in production
2. Recommend environment variables for all deployments
3. CLI arguments useful for testing only

**Rationale**: Environment variables not visible in `ps aux` or `/proc/<pid>/cmdline` (protection against local privilege escalation attacks).

## Configuration File Security

### File Permissions

**Recommended Permissions**:

- Configuration files: `644` (readable by service user, world-readable acceptable since no secrets)
- Data directory: `755` (readable and executable for directory traversal)
- Database file: `600` or `640` (readable only by service user, optionally group)
- TLS private keys: `600` (readable only by service user)

**Rationale**: Configuration files contain no secrets (validation enforcement), so world-readable acceptable. Database and TLS keys contain sensitive data, require restrictive permissions.

### Configuration File Locations

**Default Search Paths** (first found wins):

1. Path specified by `--config-file` CLI flag
2. `$TK_CONFIG_FILE` environment variable
3. `./trapperkeeper.toml` (current directory)
4. `/etc/trapperkeeper/trapperkeeper.toml` (system-wide)
5. `~/.config/trapperkeeper/trapperkeeper.toml` (user-specific)

**Security Considerations**:

- Current directory search (`./trapperkeeper.toml`): Risk of config file injection if running in untrusted directory
- Mitigation: Use absolute path via `--config-file` or `TK_CONFIG_FILE` for production

## Key Management Integration

### HMAC Secret Lifecycle

**Cross-Reference**: [Encryption Strategy](encryption.md) Section on HMAC Secrets for complete key management lifecycle.

**Summary**:

- Generation: Cryptographically secure 256-bit random (development) or operator-provided (production)
- Storage: SHA256 hashed before database persistence (environment secrets) or database-stored hash (auto-generated)
- Rotation: Dual-secret pattern enables zero-downtime migration
- Revocation: Remove environment variable, restart service (old API keys invalidated)

### TLS Private Key Protection

**Cross-Reference**: [TLS/HTTPS Strategy](tls-https-strategy.md) Section on Certificate Management for complete procedures.

**Summary**:

- Storage: File system with permissions 600 (readable only by service user)
- Rotation: Manual renewal, service restart required
- Passphrase protection: Optional (encrypt private key, provide passphrase via environment variable)

## Operational Deployment Guidance

### Container Orchestration (Kubernetes, Docker Compose)

**Kubernetes Secrets**:

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: trapperkeeper-secrets
type: Opaque
data:
  hmac-secret: <base64-encoded secret>
  database-url: <base64-encoded connection string>
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: trapperkeeper-sensor-api
spec:
  template:
    spec:
      containers:
        - name: sensor-api
          image: trapperkeeper:latest
          env:
            - name: TK_HMAC_SECRET
              valueFrom:
                secretKeyRef:
                  name: trapperkeeper-secrets
                  key: hmac-secret
            - name: TK_DATABASE_URL
              valueFrom:
                secretKeyRef:
                  name: trapperkeeper-secrets
                  key: database-url
```

**Docker Compose Secrets**:

```yaml
version: "3.8"
services:
  sensor-api:
    image: trapperkeeper:latest
    environment:
      - TK_HMAC_SECRET=${TK_HMAC_SECRET}
      - TK_DATABASE_URL=${TK_DATABASE_URL}
    env_file:
      - secrets.env # Not committed to version control
```

### Cloud Secret Managers (AWS, GCP, Azure)

**AWS Secrets Manager**:

```bash
# Retrieve secret from AWS Secrets Manager
export TK_HMAC_SECRET=$(aws secretsmanager get-secret-value \
  --secret-id trapperkeeper/hmac-secret \
  --query SecretString \
  --output text)

# Start service with environment variable
trapperkeeper sensor-api
```

**Integration**: Use init containers or sidecar patterns to fetch secrets before service startup.

## Edge Cases and Limitations

**Known Limitations**:

- Auto-generated secrets: Stored in database (database backup critical for API key validation)
- Configuration file format validation: Malformed TOML fails startup (no graceful degradation)
- Environment variable precedence: CLI > Env > File (cannot override CLI with env var)
- Secret rotation: Requires service restart (brief downtime during HMAC secret rotation)

**Edge Cases**:

- Empty environment variable: Treated as "not set" (falls back to config file or defaults)
- Multiple config files: Only first found used (no merging across multiple files)
- HMAC secret in both env var and CLI: CLI takes precedence (logged at WARN level)
- Configuration file not found: Uses defaults (no error unless required field missing)

## Related Documents

**Dependencies** (read these first):

- [Configuration Management](../09-operations/configuration.md): Complete configuration precedence rules, variable interpolation, Figment migration
- [Validation Hub](../07-validation/README.md): Configuration validation rules and structured error messages

**Related Spokes** (siblings in this hub):

- [Authentication (Sensor API)](authentication-sensor-api.md): HMAC secret usage and rotation procedures
- [Encryption Strategy](encryption.md): HMAC secret lifecycle (Section on HMAC Secrets), key storage security

**Extended by**:

- Infrastructure-as-code templates (Kubernetes, Terraform) for production deployments
- Secret rotation automation scripts (when implemented post-MVP)
