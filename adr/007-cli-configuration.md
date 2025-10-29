# ADR-007: Command Line Interface and Configuration

Date: 2025-10-28

## Related Decisions

**Depends on:**
- **ADR-006: Service Architecture** - Configures the two services (tk-sensor-api and tk-web-ui) defined in the service architecture

## Context

TrapperKeeper needs to handle configuration from multiple sources (command-line flags, environment variables, and optionally config files) in a way that's DevOps-friendly. The standard library's `flag` package only handles command-line arguments, requiring manual coordination with environment variables. We prefer minimal libraries over frameworks where possible.

The system consists of two distinct services:
- **`tk-sensor-api`**: gRPC service for sensor communication
- **`tk-web-ui`**: HTTP service for web interface with server-side rendering

Options considered:
- Standard library `flag` + manual env var handling - requires too much boilerplate for env var mapping and config file parsing
- `spf13/cobra` + `spf13/viper` - Cobra is overkill for our needs (subcommands, help generation), adds unnecessary complexity
- `spf13/viper` alone - configuration-focused library that handles our exact needs
- `koanf` - newer alternative but less mature ecosystem and documentation

## Decision

Use `spf13/viper` for all configuration management without Cobra. Create separate binaries for different services (`tk-sensor-api` and `tk-web-ui`).

## Configuration Structure

### Command-Line Flags

Core flags supported by both services:
- `--data-dir`: Base path for data storage (e.g., `/var/lib/trapperkeeper`)
  - Events stored in subdirectory: `{data-dir}/events/`
  - SQLite database: `{data-dir}/trapperkeeper.db`
- `--db-url`: Database connection string
- `--port`: Service port (different defaults per service)

### Environment Variables

The system uses environment variables with the `TK_` prefix. Viper automatically maps these to configuration keys:
- `TK_DATA_DIR` maps to `--data-dir` flag
- `TK_PORT` maps to `--port` flag
- `TK_DB_URL` maps to `--db-url` flag

### Shared Configuration

Both services share certain configuration elements:
- Database connection settings
- Data directory for file storage
- Base system configuration

Service-specific configuration is handled through separate flag defaults and environment variable namespaces where needed.

## Consequences

**Pros:**
- Unified configuration from flags, env vars, and config files
- DevOps-friendly with 12-factor app principles
- Automatic environment variable binding (e.g., `TK_PORT` maps to `port`)
- Clean separation between sensor API (gRPC) and web UI (HTTP) services
- No heavyweight CLI framework needed
- Still relatively minimal (configuration-focused, not a framework)

**Cons:**
- Need separate binaries instead of subcommands
- Must coordinate shared configuration between services

This gives us professional configuration management while keeping the architecture simple and Go-like.
