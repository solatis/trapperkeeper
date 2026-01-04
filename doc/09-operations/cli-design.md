---
doc_type: spoke
status: active
primary_category: deployment
hub_document: doc/09-operations/README.md
tags:
  - cli
  - cobra
  - subcommands
  - configuration
---

# CLI Design

## Context

TrapperKeeper requires a DevOps-friendly command-line interface for service management, database migrations, and configuration. This document specifies the cobra-based CLI design with subcommands for `sensor-api`, `web-ui`, and `migrate` operations.

**Hub Document**: This spoke is part of [Operations Overview](README.md). See the hub's Unified CLI with Subcommands section for strategic context.

## cobra Framework Selection

### Framework Rationale

**Selected**: cobra for Go

**Benefits**:

- Built-in support for subcommands and persistent flags
- Runtime validation of CLI structure with clear error messages
- Excellent help text generation and error messages out of the box
- Minimal library approach (CLI parser only, not a framework)
- Industry-standard (used by kubectl, hugo, docker, git-lfs)

**Alternatives Evaluated**:

- **flag (stdlib)**: Too basic, lacks subcommand support
- **urfave/cli**: Less flexible command structure, smaller ecosystem

### Command Definition Pattern

```go
package main

import (
    "github.com/spf13/cobra"
    "path/filepath"
)

var (
    configFile string
    dataDir    string
    dbURL      string
    logLevel   string
    logFormat  string
)

var rootCmd = &cobra.Command{
    Use:   "trapperkeeper",
    Short: "TrapperKeeper Event Processing System",
    Long:  "TrapperKeeper Event Processing System",
}

func init() {
    // Global flags available to all subcommands
    rootCmd.PersistentFlags().StringVar(&configFile, "config-file", "", "Path to configuration file")
    rootCmd.PersistentFlags().StringVar(&dataDir, "data-dir", "", "Data storage directory")
    rootCmd.PersistentFlags().StringVar(&dbURL, "db-url", "", "Database connection URL")
    rootCmd.PersistentFlags().StringVar(&logLevel, "log-level", "info", "Logging level (trace|debug|info|warn|error)")
    rootCmd.PersistentFlags().StringVar(&logFormat, "log-format", "json", "Log output format (json|pretty)")

    // Register subcommands
    rootCmd.AddCommand(sensorAPICmd)
    rootCmd.AddCommand(webUICmd)
    rootCmd.AddCommand(migrateCmd)
}
```

**Runtime Validation**: Invalid CLI structure detected at runtime with clear error messages.

## Subcommand Architecture

### Three Core Subcommands

**1. sensor-api**: Start gRPC sensor API service

```bash
trapperkeeper sensor-api [FLAGS]
```

**2. web-ui**: Start HTTP web UI service

```bash
trapperkeeper web-ui [FLAGS]
```

**3. migrate**: Run database migrations

```bash
trapperkeeper migrate [FLAGS]
```

**Rationale**: Single binary with subcommands simplifies distribution and versioning.

**Cross-Reference**: See [Architecture: Binary Distribution](../02-architecture/binary-distribution.md) for complete single-binary strategy.

### Subcommand Routing

```go
func main() {
    if err := rootCmd.Execute(); err != nil {
        fmt.Fprintln(os.Stderr, err)
        os.Exit(1)
    }
}

var sensorAPICmd = &cobra.Command{
    Use:   "sensor-api",
    Short: "Start sensor API service (gRPC)",
    RunE: func(cmd *cobra.Command, args []string) error {
        globalCfg := buildGlobalConfig()
        return sensorapi.Run(globalCfg, sensorAPIArgs)
    },
}

var webUICmd = &cobra.Command{
    Use:   "web-ui",
    Short: "Start web UI service (HTTP)",
    RunE: func(cmd *cobra.Command, args []string) error {
        globalCfg := buildGlobalConfig()
        return webui.Run(globalCfg, webUIArgs)
    },
}
```

**Shared Configuration**: Global flags accessible to all subcommands via persistent flags.

## Global Flags

### Shared Configuration Flags

Available to ALL subcommands:

```bash
trapperkeeper [GLOBAL_FLAGS] <SUBCOMMAND> [SUBCOMMAND_FLAGS]

Global Flags:
  --config-file <PATH>        Path to configuration file
  --data-dir <PATH>           Data storage directory
  --db-url <URL>              Database connection string
  --log-level <LEVEL>         Logging level (trace|debug|info|warn|error)
  --log-format <FORMAT>       Log output format (json|pretty)
```

**Example Usage**:

```bash
# Override configuration file
trapperkeeper --config-file /custom/config.toml sensor-api

# Set log level for debugging
trapperkeeper --log-level debug web-ui

# Override database URL
trapperkeeper --db-url "postgresql://localhost/trapperkeeper" migrate
```

### Global Flag Implementation

```go
type GlobalConfig struct {
    ConfigFile string
    DataDir    string
    DBURL      string
    LogLevel   string
    LogFormat  string
}

func buildGlobalConfig() GlobalConfig {
    return GlobalConfig{
        ConfigFile: configFile,
        DataDir:    dataDir,
        DBURL:      dbURL,
        LogLevel:   logLevel,
        LogFormat:  logFormat,
    }
}
```

**Shared State**: Global flags passed to subcommand handlers via `GlobalConfig` struct.

## sensor-api Subcommand

### Command Structure

```bash
trapperkeeper sensor-api [FLAGS]

Start sensor API service (gRPC)

Options:
  --host <HOST>               Bind address [default: 0.0.0.0]
  --port <PORT>               gRPC port [default: 50051]
  --hmac-secret <SECRET>      HMAC secret for API authentication (security-sensitive)
  --max-connections <NUM>     Maximum concurrent connections [default: 1000]
  --request-timeout <MS>      Request timeout in milliseconds [default: 5000]

Global Options:
  --config-file <PATH>        Path to configuration file
  --data-dir <PATH>           Data storage directory
  --db-url <URL>              Database connection string
  --log-level <LEVEL>         Logging level [default: info]
  --log-format <FORMAT>       Log output format [default: json]

  -h, --help                  Print help
```

### sensor-api Arguments

```go
var sensorAPIArgs struct {
    host           string
    port           int
    hmacSecret     string
    maxConnections int
    requestTimeout int
}

func init() {
    sensorAPICmd.Flags().StringVar(&sensorAPIArgs.host, "host", "0.0.0.0", "Bind address")
    sensorAPICmd.Flags().IntVar(&sensorAPIArgs.port, "port", 50051, "gRPC port")
    sensorAPICmd.Flags().StringVar(&sensorAPIArgs.hmacSecret, "hmac-secret", "", "HMAC secret for API authentication")
    sensorAPICmd.Flags().IntVar(&sensorAPIArgs.maxConnections, "max-connections", 1000, "Maximum concurrent connections")
    sensorAPICmd.Flags().IntVar(&sensorAPIArgs.requestTimeout, "request-timeout", 5000, "Request timeout in milliseconds")
}
```

### Example Usage

```bash
# Start with defaults
export TK_HMAC_SECRET="<secret>"
trapperkeeper sensor-api

# Override port and max connections
trapperkeeper sensor-api --port 9090 --max-connections 2000

# Use custom configuration file
trapperkeeper --config-file /custom/config.toml sensor-api

# Development mode with debug logging
trapperkeeper --log-level debug sensor-api --port 50051
```

**Security Note**: HMAC secret should be provided via `TK_HMAC_SECRET` environment variable, not CLI flag (visible in process list).

**Cross-Reference**: See [Security: Configuration Security](../06-security/configuration-security.md) for secrets management.

## web-ui Subcommand

### Command Structure

```bash
trapperkeeper web-ui [FLAGS]

Start web UI service (HTTP)

Options:
  --host <HOST>               Bind address [default: 0.0.0.0]
  --port <PORT>               HTTP port [default: 8080]
  --session-max-age <HOURS>   Session cookie max age in hours [default: 168]
  --csrf-enabled <BOOL>       Enable CSRF protection [default: true]

Global Options:
  --config-file <PATH>        Path to configuration file
  --data-dir <PATH>           Data storage directory
  --db-url <URL>              Database connection string
  --log-level <LEVEL>         Logging level [default: info]
  --log-format <FORMAT>       Log output format [default: json]

  -h, --help                  Print help
```

### web-ui Arguments

```go
var webUIArgs struct {
    host          string
    port          int
    sessionMaxAge int
    csrfEnabled   bool
}

func init() {
    webUICmd.Flags().StringVar(&webUIArgs.host, "host", "0.0.0.0", "Bind address")
    webUICmd.Flags().IntVar(&webUIArgs.port, "port", 8080, "HTTP port")
    webUICmd.Flags().IntVar(&webUIArgs.sessionMaxAge, "session-max-age", 168, "Session cookie max age in hours")
    webUICmd.Flags().BoolVar(&webUIArgs.csrfEnabled, "csrf-enabled", true, "Enable CSRF protection")
}
```

### Example Usage

```bash
# Start with defaults
trapperkeeper web-ui

# Override port
trapperkeeper web-ui --port 8443

# Disable CSRF for testing (NOT RECOMMENDED FOR PRODUCTION)
trapperkeeper web-ui --csrf-enabled false

# Use custom configuration file
trapperkeeper --config-file /custom/config.toml web-ui

# Production deployment with PostgreSQL
trapperkeeper --db-url "postgresql://localhost/trapperkeeper" web-ui --port 8080
```

## migrate Subcommand

### Command Structure

```bash
trapperkeeper migrate [FLAGS]

Run database migrations

Options:
  --db-url <URL>              Database connection URL (required if not in config)

Global Options:
  --config-file <PATH>        Path to configuration file
  --data-dir <PATH>           Data storage directory
  --log-level <LEVEL>         Logging level [default: info]
  --log-format <FORMAT>       Log output format [default: json]

  -h, --help                  Print help
```

### migrate Arguments

```go
// No additional flags beyond global persistent flags
var migrateCmd = &cobra.Command{
    Use:   "migrate",
    Short: "Run database migrations",
    RunE: func(cmd *cobra.Command, args []string) error {
        globalCfg := buildGlobalConfig()
        return migrate.Run(globalCfg)
    },
}
```

**Database URL Required**: Either via `--db-url` flag or configuration file.

### Example Usage

```bash
# SQLite migration
trapperkeeper migrate --db-url "sqlite:///var/lib/trapperkeeper/trapperkeeper.db"

# PostgreSQL migration
trapperkeeper migrate --db-url "postgresql://user:pass@localhost/trapperkeeper"

# Use configuration file for database URL
trapperkeeper --config-file /custom/config.toml migrate
```

**Cross-Reference**: See [Database Migrations](database-migrations.md) for complete migration strategy.

## Help Text Generation

### Automatic Help Text

cobra generates help text automatically from command definitions:

```bash
# Top-level help
trapperkeeper --help

# Subcommand help
trapperkeeper sensor-api --help
trapperkeeper web-ui --help
trapperkeeper migrate --help
```

### Help Text Example

```
TrapperKeeper Event Processing System

Usage:
  trapperkeeper [command]

Available Commands:
  sensor-api    Start sensor API service (gRPC)
  web-ui        Start web UI service (HTTP)
  migrate       Run database migrations
  help          Help about any command

Flags:
      --config-file string   Path to configuration file
      --data-dir string      Data storage directory
      --db-url string        Database connection string
      --log-level string     Logging level (default "info")
      --log-format string    Log output format (default "json")
  -h, --help                 help for trapperkeeper

Use "trapperkeeper [command] --help" for more information about a command.
```

**Custom Descriptions**: Use Long field for detailed help text:

```go
sensorAPICmd := &cobra.Command{
    Use:   "sensor-api",
    Short: "Start sensor API service (gRPC)",
    Long: `Start sensor API service (gRPC)

The sensor-api service accepts event submissions from TrapperKeeper SDKs
via gRPC protocol with HMAC-based authentication.`,
    RunE: sensorAPIRun,
}
```

## Error Reporting

### cobra Error Messages

cobra provides clear error messages automatically:

**Invalid Flag**:

```bash
trapperkeeper sensor-api --invalid-flag
# → Error: unknown flag: --invalid-flag
#   Usage:
#     trapperkeeper sensor-api [flags]
```

**Missing Required Value**:

```bash
trapperkeeper sensor-api --port
# → Error: flag needs an argument: --port
```

**Invalid Type**:

```bash
trapperkeeper sensor-api --port not-a-number
# → Error: invalid argument "not-a-number" for "--port" flag: strconv.ParseInt: parsing "not-a-number": invalid syntax
```

### Custom Validation Errors

Custom validation with clear error messages:

```go
func validatePort(port int) error {
    if port < 1024 && !isPrivilegedUser() {
        return fmt.Errorf(
            "port %d requires root privileges. Use port >= 1024 or run as root",
            port,
        )
    }
    return nil
}
```

**Example Error**:

```bash
trapperkeeper sensor-api --port 80
# → Error: port 80 requires root privileges. Use port >= 1024 or run as root
```

**Cross-Reference**: See [Validation: Unified Validation and Input Sanitization](../07-validation/README.md) Section 5.2 for complete error message format.

## Configuration Integration

### Multi-Source Configuration

CLI arguments have highest precedence in configuration hierarchy:

**Precedence Order**: CLI > Environment > Config File > Defaults

**Example**:

```toml
# trapperkeeper.toml
[sensor_api]
port = 50051
max_connections = 1000
```

```bash
export TK_SENSOR_API_MAX_CONNECTIONS=2000
trapperkeeper sensor-api --port 9090

# Result:
# - port = 9090 (CLI)
# - max_connections = 2000 (Environment)
```

**Cross-Reference**: See [Configuration Management](configuration.md) for complete precedence rules.

### Flag-to-Configuration Mapping

CLI flags map to configuration sections:

```bash
# Global flags → [common] section
--data-dir       → common.data_dir
--db-url         → common.db_url
--log-level      → common.log_level

# sensor-api flags → [sensor_api] section
--host           → sensor_api.host
--port           → sensor_api.port
--hmac-secret    → sensor_api.hmac_secret

# web-ui flags → [web_ui] section
--host           → web_ui.host
--port           → web_ui.port
--csrf-enabled   → web_ui.csrf_enabled
```

## Related Documents

**Dependencies** (read these first):

- [Operations Overview](README.md): Strategic context for unified CLI
- [Configuration Management](configuration.md): CLI argument precedence and mapping

**Related Spokes** (siblings in this hub):

- [Database Migrations](database-migrations.md): `migrate` subcommand usage

**Architecture References**:

- [Architecture: Binary Distribution](../02-architecture/binary-distribution.md): Single binary with subcommands architecture
- [Architecture: Service Architecture](../02-architecture/README.md): Two-service model with `sensor-api` and `web-ui` subcommands

**Security References**:

- [Security: Configuration Security](../06-security/configuration-security.md): CLI argument security and secrets management
