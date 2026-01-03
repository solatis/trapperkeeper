---
doc_type: spoke
status: active
date_created: 2025-11-07
primary_category: configuration
hub_document: doc/09-operations/README.md
tags:
  - configuration
  - toml
  - environment-variables
  - cli-arguments
  - viper
---

# Configuration Management

## Context

TrapperKeeper requires flexible configuration supporting multiple deployment scenarios: development (file-based convenience), container orchestration (environment variables), and production (CLI overrides). This document specifies operational configuration mechanics including file formats, environment variable mappings, CLI argument precedence, and validation rules.

**Hub Document**: This spoke is part of [Operations Overview](README.md). See the hub's Multi-Source Configuration section for strategic context.

**Security Note**: For secrets management policies and enforcement, see [Security: Configuration Security](../06-security/configuration-security.md). This document covers ONLY operational mechanics (formats, precedence, validation).

## TOML Configuration Format

### File Structure and Syntax

TrapperKeeper uses TOML for configuration files with section-based organization:

```toml
# trapperkeeper.toml - Complete configuration example

# Shared configuration for both services
[common]
data_dir = "/var/lib/trapperkeeper"
db_url = "sqlite://{data_dir}/trapperkeeper.db"  # Variable interpolation
log_level = "info"  # trace, debug, info, warn, error
log_format = "json"  # json or pretty

# Sensor API service (gRPC)
[sensor_api]
enabled = true
host = "0.0.0.0"
port = 50051
max_connections = 1000
request_timeout_ms = 5000

# Web UI service (HTTP)
[web_ui]
enabled = true
host = "0.0.0.0"
port = 8080
session_cookie_name = "tk_session"
session_max_age_hours = 168  # 7 days
csrf_enabled = true

# Database connection pooling
[database]
max_connections = 10
connection_timeout_ms = 3000
idle_timeout_ms = 300000  # 5 minutes
migrations_on_startup = true

# Performance tuning
[performance]
batch_size = 1000
cache_enabled = true
cache_ttl_seconds = 300
```

### Variable Interpolation

Configuration files support variable interpolation using `{variable}` syntax:

```toml
[common]
data_dir = "/var/lib/trapperkeeper"

[database]
# References {data_dir} from [common] section
db_url = "sqlite://{data_dir}/trapperkeeper.db"
```

**Resolution Order**:

1. Configuration file values (same section first, then other sections)
2. Environment variables
3. Built-in variables (`{hostname}`, `{pid}`)

**Example with Built-ins**:

```toml
[common]
log_file = "/var/log/trapperkeeper/{hostname}-{pid}.log"
# Result: /var/log/trapperkeeper/server01-12345.log
```

### Section Organization

**[common]**: Shared fields for all services

- `data_dir`: Base directory for data storage
- `db_url`: Database connection string
- `log_level`: Logging verbosity
- `log_format`: Log output format

**[sensor_api]**: Sensor API service (gRPC)

- `host`: Bind address
- `port`: gRPC port (default 50051)
- `max_connections`: Concurrent connection limit
- `request_timeout_ms`: Request timeout

**[web_ui]**: Web UI service (HTTP)

- `host`: Bind address
- `port`: HTTP port (default 8080)
- `session_cookie_name`: Session cookie identifier
- `session_max_age_hours`: Session duration
- `csrf_enabled`: CSRF protection toggle

**[database]**: Database connection pooling

- `max_connections`: Pool size
- `connection_timeout_ms`: Connection establishment timeout
- `idle_timeout_ms`: Idle connection timeout

**[performance]**: Performance tuning

- `batch_size`: Batch operation size
- `cache_enabled`: Enable caching
- `cache_ttl_seconds`: Cache entry lifetime

## Default File Paths

Configuration files loaded from these locations (first found wins):

1. **`--config-file` CLI flag**: Explicit path (highest priority)
2. **`$TK_CONFIG_FILE` environment variable**: Override default search
3. **`./trapperkeeper.toml`**: Current directory
4. **`/etc/trapperkeeper/trapperkeeper.toml`**: System-wide
5. **`~/.config/trapperkeeper/trapperkeeper.toml`**: User-specific

**Search Behavior**:

- Stops at first existing file
- No merging across multiple files
- If no file found: Uses defaults (no error unless required field missing)

**Example Search**:

```bash
# Explicit path (highest priority)
./trapperkeeper sensor-api --config-file /custom/path/config.toml

# Environment variable override
export TK_CONFIG_FILE=/etc/trapperkeeper/prod.toml
./trapperkeeper sensor-api

# Default search (checks ./trapperkeeper.toml, then /etc/..., then ~/.config/...)
./trapperkeeper sensor-api
```

## Environment Variable Mapping

### Naming Convention

Format: `TK_<SECTION>_<KEY>` (all uppercase, underscores separate words)

**Mapping Rules**:

- Section name converted to uppercase
- Field name converted to snake_case
- Nested fields use additional underscores

**Examples**:

```bash
# [common] section
export TK_COMMON_DATA_DIR="/var/lib/trapperkeeper"
export TK_COMMON_DB_URL="postgresql://localhost/trapperkeeper"
export TK_COMMON_LOG_LEVEL="debug"

# [sensor_api] section
export TK_SENSOR_API_PORT="50051"
export TK_SENSOR_API_HOST="0.0.0.0"
export TK_SENSOR_API_MAX_CONNECTIONS="2000"

# [web_ui] section
export TK_WEB_UI_PORT="8080"
export TK_WEB_UI_CSRF_ENABLED="true"

# [database] section
export TK_DATABASE_MAX_CONNECTIONS="20"
```

### Special Environment Variables

**Configuration File Path**:

```bash
export TK_CONFIG_FILE="/custom/path/config.toml"
```

**Secrets** (security-sensitive, NEVER in configuration files):

```bash
# HMAC secret for API authentication (single secret)
export TK_HMAC_SECRET="<256-bit hex string>"

# HMAC secrets for rotation (multiple secrets)
export TK_HMAC_SECRET_1="<old secret>"
export TK_HMAC_SECRET_2="<new secret>"
```

**Cross-Reference**: See [Security: Configuration Security](../06-security/configuration-security.md) for complete secrets management policies.

### Type Coercion

Environment variables parsed with automatic type conversion:

**Boolean**:

- Accepted: `true`, `false`, `1`, `0`, `yes`, `no` (case-insensitive)
- Example: `TK_WEB_UI_CSRF_ENABLED=true` or `TK_WEB_UI_CSRF_ENABLED=1`

**Integer**:

- Numeric strings only
- Reject non-numeric values with validation error
- Example: `TK_SENSOR_API_PORT=50051`

**Duration**:

- Suffixes: `ms` (milliseconds), `s` (seconds), `m` (minutes), `h` (hours)
- Examples:
  - `TK_DATABASE_CONNECTION_TIMEOUT_MS=5000` (raw milliseconds)
  - `TK_DATABASE_CONNECTION_TIMEOUT=5s` (5 seconds)
  - `TK_SESSION_MAX_AGE=24h` (24 hours)

**Arrays**:

- Comma-separated values
- Example: `TK_ALLOWED_ORIGINS="http://localhost:3000,http://localhost:8080"`

**Empty Values**:

- Empty string treated as "not set" (falls back to config file or defaults)
- Example: `TK_SENSOR_API_PORT=""` → Uses default port 50051

## CLI Argument Mapping

### Global Flags

Available to all subcommands:

```bash
trapperkeeper [GLOBAL_FLAGS] <SUBCOMMAND> [SUBCOMMAND_FLAGS]

Global Flags:
  --config-file <PATH>        Path to configuration file
  --data-dir <PATH>           Data storage directory
  --db-url <URL>              Database connection string
  --log-level <LEVEL>         Logging level (trace|debug|info|warn|error)
  --log-format <FORMAT>       Log output format (json|pretty)
```

**Examples**:

```bash
# Override configuration file location
trapperkeeper --config-file /custom/config.toml sensor-api

# Override database URL
trapperkeeper --db-url "postgresql://localhost/trapperkeeper" sensor-api

# Set log level
trapperkeeper --log-level debug sensor-api
```

### Sensor API Subcommand

```bash
trapperkeeper sensor-api [FLAGS]

Flags:
  --host <HOST>               Bind address (default: 0.0.0.0)
  --port <PORT>               gRPC port (default: 50051)
  --hmac-secret <SECRET>      HMAC secret for API authentication (security-sensitive)
  --max-connections <NUM>     Maximum concurrent connections
  --request-timeout <MS>      Request timeout in milliseconds
```

**Example**:

```bash
export TK_HMAC_SECRET="<secret>"
trapperkeeper sensor-api --port 9090 --max-connections 2000
# Result: port=9090 (CLI), max_connections=2000 (CLI), hmac_secret from env
```

### Web UI Subcommand

```bash
trapperkeeper web-ui [FLAGS]

Flags:
  --host <HOST>               Bind address (default: 0.0.0.0)
  --port <PORT>               HTTP port (default: 8080)
  --session-max-age <HOURS>   Session cookie max age in hours
  --csrf-enabled <BOOL>       Enable CSRF protection (default: true)
```

**Example**:

```bash
trapperkeeper web-ui --port 8443 --csrf-enabled true
# Result: port=8443 (CLI), csrf_enabled=true (CLI)
```

### Kebab-Case Convention

CLI arguments use kebab-case (hyphen-separated):

- `--data-dir` (NOT `--data_dir`)
- `--max-connections` (NOT `--max_connections`)
- `--session-max-age` (NOT `--session_max_age`)

**Rationale**: Standard Unix convention for command-line tools.

## Configuration Precedence

### Precedence Order

**Explicit Hierarchy** (highest priority wins):

1. **CLI arguments** (highest)
2. **Environment variables**
3. **Configuration file**
4. **Defaults** (lowest)

### Precedence Examples

#### Example 1: Database URL Resolution

```toml
# trapperkeeper.toml
[common]
db_url = "sqlite:///var/lib/trapperkeeper/trapperkeeper.db"
```

```bash
export TK_COMMON_DB_URL="postgresql://localhost/trapperkeeper"
trapperkeeper sensor-api --db-url "sqlite://./dev.db"
```

**Result**: `sqlite://./dev.db` (CLI overrides environment overrides file)

#### Example 2: Mixed Configuration Sources

```toml
# trapperkeeper.toml
[sensor_api]
port = 50051
max_connections = 1000
```

```bash
export TK_SENSOR_API_MAX_CONNECTIONS=2000
trapperkeeper sensor-api --port 9090
```

**Result**:

- `port = 9090` (from CLI argument)
- `max_connections = 2000` (from environment variable)

#### Example 3: Secrets Enforcement

```toml
# trapperkeeper.toml - INVALID
[sensor_api]
hmac_secret = "this-will-be-rejected"  # ERROR: Secrets not allowed in files
```

**Result**: Configuration validation error at startup:

```
Error: Configuration validation failed
  - sensor_api.hmac_secret: Secret values cannot be stored in configuration files
    Use TK_HMAC_SECRET environment variable or --hmac-secret CLI flag instead
```

**Cross-Reference**: See [Security: Configuration Security](../06-security/configuration-security.md) for complete secrets rejection policy.

## Type Validation

**NOTE**: Type validation rules (port ranges, timeout validation, boolean coercion, path canonicalization, database URLs, log levels, host addresses) are authoritative in [Validation: Unified Validation and Input Sanitization](../07-validation/README.md) Section 3.5.

This section provides implementation context specific to configuration loading.

### Port Number Validation

```bash
# Valid
trapperkeeper sensor-api --port 50051

# Invalid: Out of range
trapperkeeper sensor-api --port 99999
# → Error: Invalid port number '99999'. Must be between 1 and 65535.
```

### Timeout Validation

```bash
# Valid
trapperkeeper sensor-api --request-timeout 5000  # Milliseconds

# Valid with duration suffix
export TK_SENSOR_API_REQUEST_TIMEOUT=5s

# Invalid: Negative value
trapperkeeper sensor-api --request-timeout -1
# → Error: Timeout values must be non-negative
```

### Path Validation

```bash
# Valid: Path exists
trapperkeeper sensor-api --data-dir /var/lib/trapperkeeper

# Invalid: Parent directory doesn't exist
trapperkeeper sensor-api --data-dir /nonexistent/path/data
# → Error: Data directory parent '/nonexistent/path' does not exist
```

### Database URL Validation

```bash
# Valid SQLite
trapperkeeper sensor-api --db-url "sqlite:///var/lib/trapperkeeper/trapperkeeper.db"

# Valid PostgreSQL
trapperkeeper sensor-api --db-url "postgresql://user:pass@localhost/trapperkeeper"

# Invalid: Malformed URL
trapperkeeper sensor-api --db-url "not-a-valid-url"
# → Error: Invalid database URL format
```

### Log Level Validation

```bash
# Valid
trapperkeeper sensor-api --log-level debug

# Invalid: Unknown level
trapperkeeper sensor-api --log-level verbose
# → Error: Invalid log level 'verbose'. Valid values: trace, debug, info, warn, error
```

## Dependency Validation

**NOTE**: Dependency validation rules (Database URL + Data Directory, CSRF Enabled + Session Cookie, HMAC Secret, TLS Certificate + Key) are authoritative in [Validation: Unified Validation and Input Sanitization](../07-validation/README.md) Section 3.5.

This section provides configuration-specific context.

### Database URL + Data Directory

**Rule**: If `db_url` uses SQLite, `data_dir` must be configured (database file requires parent directory).

**Valid**:

```bash
trapperkeeper sensor-api --data-dir /var/lib/trapperkeeper --db-url "sqlite:///var/lib/trapperkeeper/db.sqlite"
```

**Invalid**:

```bash
trapperkeeper sensor-api --db-url "sqlite:///var/lib/trapperkeeper/db.sqlite"
# (No --data-dir specified)
# → Error: SQLite database URL requires data_dir to be configured
```

### HMAC Secret Requirement

**Rule**: If `sensor-api` subcommand used, HMAC secret MUST be provided via environment variable or CLI argument.

**Valid**:

```bash
export TK_HMAC_SECRET="<256-bit hex string>"
trapperkeeper sensor-api
```

**Invalid**:

```bash
trapperkeeper sensor-api
# (No TK_HMAC_SECRET environment variable)
# → Error: HMAC secret required for sensor-api. Set TK_HMAC_SECRET environment variable or use --hmac-secret flag
```

## Configuration Schema Generation

Configuration schema can be generated from Go struct definitions using JSON schema tools:

```go
package config

type TrapperKeeperConfig struct {
    Common    CommonConfig    `json:"common" mapstructure:"common"`
    SensorAPI SensorAPIConfig `json:"sensor_api" mapstructure:"sensor_api"`
    WebUI     WebUIConfig     `json:"web_ui" mapstructure:"web_ui"`
    Database  DatabaseConfig  `json:"database" mapstructure:"database"`
}

type CommonConfig struct {
    // Base directory for data storage
    DataDir string `json:"data_dir" mapstructure:"data_dir"`

    // Database connection URL
    DBURL string `json:"db_url" mapstructure:"db_url"`

    // Logging level (trace|debug|info|warn|error)
    LogLevel string `json:"log_level" mapstructure:"log_level"`
}
```

**Schema Generation**: Use `go-jsonschema` or similar tools to generate JSON schema from struct tags.

**Schema Usage**:

- Editor autocompletion (VS Code with YAML/TOML extensions)
- Configuration file validation in CI/CD pipelines
- Documentation generation
- Configuration UI builders

## Viper Migration Guide

This section provides step-by-step migration path from MVP CLI-only configuration to multi-source configuration using viper.

### Phase 1: Add Viper Dependency

**Goal**: Introduce viper without breaking existing CLI-only configuration.

```bash
go get github.com/spf13/viper
```

**Testing Strategy**: All existing CLI tests must pass without modification.

### Phase 2: Define Configuration Precedence Pipeline

**Goal**: Establish viper precedence pipeline matching this document's precedence order.

```go
import "github.com/spf13/viper"

func loadConfig() (*TrapperKeeperConfig, error) {
    v := viper.New()

    // Lowest priority: Configuration file
    v.SetConfigName("trapperkeeper")
    v.SetConfigType("toml")
    v.AddConfigPath("/etc/trapperkeeper")
    v.AddConfigPath("$HOME/.config/trapperkeeper")
    v.AddConfigPath(".")

    // Read config file (ignore if not found)
    _ = v.ReadInConfig()

    // Middle priority: Environment variables
    v.SetEnvPrefix("TK")
    v.AutomaticEnv()
    v.SetEnvKeyReplacer(strings.NewReplacer(".", "_"))

    // Highest priority: CLI arguments (bound in Phase 3)

    var cfg TrapperKeeperConfig
    if err := v.Unmarshal(&cfg); err != nil {
        return nil, err
    }

    return &cfg, nil
}
```

**Validation Point**: Configuration precedence must match order: CLI > Env > File > Defaults

### Phase 3: Migrate CLI-Only Validation to Viper

**Goal**: Replace manual CLI parsing with viper-based configuration loading while preserving all validation rules.

```go
// Before (MVP): Manual CLI parsing
config := Config{
    Port: 8080,
    Host: "0.0.0.0",
}
if cliArgs.Port != 0 {
    config.Port = cliArgs.Port
}

// After (Post-MVP): Viper with CLI binding
v := viper.New()
v.BindPFlag("sensor_api.port", cmd.Flags().Lookup("port"))
v.BindPFlag("sensor_api.host", cmd.Flags().Lookup("host"))

var config TrapperKeeperConfig
v.Unmarshal(&config)
```

**Migration Checklist**:

- [ ] All CLI arguments preserved in viper configuration
- [ ] Default values match existing MVP defaults
- [ ] Validation functions reused without modification
- [ ] Error messages remain consistent
- [ ] Existing integration tests pass without changes

### Phase 4: Add Configuration File Discovery

**Goal**: Enable configuration file loading from default paths.

```go
func discoverConfigFile() string {
    // Priority order from Default File Paths section
    paths := []string{
        os.Getenv("TK_CONFIG_FILE"),
        "./trapperkeeper.toml",
        "/etc/trapperkeeper/trapperkeeper.toml",
        filepath.Join(os.Getenv("HOME"), ".config/trapperkeeper/trapperkeeper.toml"),
    }

    for _, path := range paths {
        if path == "" {
            continue
        }
        if _, err := os.Stat(path); err == nil {
            return path
        }
    }

    return ""
}
```

**Testing Strategy**: Create integration tests for each configuration path with precedence validation.

### Post-Migration Validation

After migration, verify:

1. **CLI-Only Deployments**: Services start with CLI arguments exactly as MVP (no configuration file required)
2. **Precedence Correctness**: CLI arguments override environment variables override configuration files
3. **Secrets Enforcement**: Secrets rejected in configuration files per security validation
4. **Error Message Consistency**: All validation errors follow standard format
5. **Performance**: Configuration loading adds <10ms to startup time

### Example: Complete Migration for sensor-api

**Before (MVP)**:

```bash
trapperkeeper sensor-api --port 50051 --hmac-secret "secret-key"
```

**After (Post-MVP) - Same CLI behavior**:

```bash
trapperkeeper sensor-api --port 50051 --hmac-secret "secret-key"
```

**After (Post-MVP) - Using configuration file**:

```toml
# trapperkeeper.toml
[sensor_api]
port = 50051

# HMAC secret still via CLI or env (not allowed in file)
```

```bash
export TK_HMAC_SECRET="secret-key"
trapperkeeper sensor-api --config-file trapperkeeper.toml
```

**After (Post-MVP) - Mixed sources with precedence**:

```toml
# trapperkeeper.toml
[sensor_api]
port = 50051
max_connections = 1000
```

```bash
export TK_SENSOR_API_MAX_CONNECTIONS=2000
trapperkeeper sensor-api --port 9090 --hmac-secret "secret-key"
# Result: port=9090 (CLI), max_connections=2000 (env), hmac_secret from CLI
```

## Validation Error Format

Clear, actionable error messages with remediation guidance:

```
Configuration validation failed (3 errors):

  1. sensor_api.port: Invalid port number '99999'
     → Must be between 1 and 65535

  2. common.data_dir: Directory does not exist '/nonexistent/path'
     → Create directory or update configuration

  3. sensor_api.hmac_secret: Secret values cannot be stored in configuration files
     → Use TK_HMAC_SECRET environment variable or --hmac-secret CLI flag
```

**Error Format Integration**: Error messages follow [Validation: Unified Validation and Input Sanitization](../07-validation/README.md) Section 5.2 format for consistency across all validation contexts.

## Related Documents

**Dependencies** (read these first):

- [Operations Overview](README.md): Strategic context for multi-source configuration
- [Validation: Unified Validation and Input Sanitization](../07-validation/README.md): Authoritative validation rules (Section 3.5)

**Related Spokes** (siblings in this hub):

- [CLI Design](cli-design.md): CLI argument parsing with cobra
- [Database Backend](database-backend.md): Database URL format and connection strings

**Security Policies** (configuration security):

- [Security: Configuration Security](../06-security/configuration-security.md): Secrets rejection policies, environment variable security, HMAC secret management

**Extended by**:

- Infrastructure-as-code templates (Kubernetes, Terraform) for production deployments
