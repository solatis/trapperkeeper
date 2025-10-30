# ADR-007: Command Line Interface and Configuration

## Revision log

| Date | Description |
|------|-------------|
| 2025-10-28 | Document created |

## Context

TrapperKeeper needs to handle configuration for command-line flags in a way that's DevOps-friendly. For the MVP, we focus on command-line arguments only; file and environment-based configuration can be added later if needed.

## Decision

We will use `clap` v4 with derive macros for command-line argument parsing by creating separate binaries for different services (`tk-sensor-api` and `tk-web-ui`).

File and environment-based configuration is not supported in the MVP. If needed later, we can integrate `figment` for unified configuration from multiple sources.

## Consequences

**Benefits:**
- Derive macros reduce boilerplate and improve maintainability
- Compile-time validation of CLI arguments prevents runtime errors
- Excellent help text generation and error messages out of the box
- Clean separation between sensor API (gRPC) and web UI (HTTP) services
- Minimal library approach (CLI parser only, not a framework)
- Can easily add file/env configuration later via `figment` if needed

**Tradeoffs:**
- Need separate binaries instead of subcommands
- Must coordinate shared configuration between services
- MVP lacks environment variable and config file support (acceptable tradeoff for simplicity)

**Operational Implications:**
- DevOps teams configure services using command-line flags only
- Each service (`tk-sensor-api` and `tk-web-ui`) requires separate binary invocation
- Shared configuration (database, data directory) must be specified for each service
- The `TK_` environment variable prefix is reserved for SDK client metadata (ADR-020), not service configuration

## Implementation

1. Add `clap` v4 with derive feature to dependencies
2. Define shared configuration struct with common fields (`--data-dir`, `--db-url`)
3. Create service-specific configuration structs for `tk-sensor-api` and `tk-web-ui`
4. Implement service-specific defaults for `--port` flag (different per service)
5. Generate separate binaries in `src/bin/tk-sensor-api.rs` and `src/bin/tk-web-ui.rs`
6. Document required command-line flags in help text and deployment guides

## Related Decisions

**Depends on:**
- **ADR-006: Service Architecture** - Configures the two services (tk-sensor-api and tk-web-ui) defined in the service architecture

## Appendix A: Configuration Structure

### Command-Line Flags

Core flags supported by both services:
- `--data-dir`: Base path for data storage (e.g., `/var/lib/trapperkeeper`)
  - Events stored in subdirectory: `{data-dir}/events/`
  - SQLite database: `{data-dir}/trapperkeeper.db`
- `--db-url`: Database connection string
- `--port`: Service port (different defaults per service)

### Environment Variables (Out of Scope for MVP)

Environment variable support is not included in the MVP. If needed later, `figment` can provide unified configuration from env vars, files, and command-line flags.

Note: The `TK_` prefix is reserved for SDK client metadata (see ADR-020: Client Metadata Namespace), not for service configuration.

### Shared Configuration

Both services share certain configuration elements:
- Database connection settings
- Data directory for file storage
- Base system configuration

Service-specific configuration is handled through separate flag defaults in each binary.

### Library Options Evaluated

Options considered:
- `clap` v4 - mature, widely-used CLI parser with derive macros, excellent ergonomics, and compile-time validation (selected)
- `structopt` (now merged into clap v4) - predecessor to clap's derive API
- `argh` - smaller but less feature-rich, limited documentation
