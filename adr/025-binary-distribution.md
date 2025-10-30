# ADR-025: Binary Distribution Strategy

## Revision Log

| Date | Description |
|------|-------------|
| 2025-10-28 | Document created |

## Context

TrapperKeeper consists of two distinct services (gRPC sensor API and HTTP web UI) that share significant infrastructure including database access, migrations, configuration management, and core business logic.

## Decision

Deploy both services from a **single binary with subcommands**.

### Architecture

The system uses a Cargo workspace with a shared tk-core library crate containing database access, migrations, authentication primitives, and core business logic. The main binary crate implements the CLI with subcommands for sensor-api, web-ui, and migrate operations.

### Subcommand Implementation

Using clap (ADR-007) with derive macros:
- `sensor-api`: Start gRPC service on specified port
- `web-ui`: Start HTTP service on specified port
- `migrate`: Execute database migrations and exit
- Common flags: `--db-url`, `--data-dir`, `--help`, `--version`

Services run as independent processes despite shared binary. Database connections use shared tk-core functions, and migrations are embedded via `sqlx::migrate!()` in the tk-core library.

## Consequences

### Benefits

1. **Simplified Release Process**: One build produces one artifact with consistent versioning
2. **Code Reuse**: Database logic, migrations, and configuration shared without duplication
3. **Deployment Flexibility**: Can still deploy services independently by using different subcommands
4. **Smaller Total Size**: Shared code means smaller combined deployment than separate binaries
5. **Version Consistency**: Services always use compatible versions of shared logic
6. **Operational Simplicity**: Single binary to distribute, track, and upgrade
7. **Standard Pattern**: Common in Rust ecosystem (cargo, rustup, tokio-console)

### Tradeoffs

1. **Larger Single Binary**: Contains code for both services even if only one is used
2. **Separate Processes Still Required**: Services don't share process memory or resources
3. **Unified Build**: Cannot build services independently (workspace builds both)
4. **All Code Deployed**: Even unused service code is present in binary

### Operational Implications

1. **Container Deployment**: Single image with different CMD/ENTRYPOINT per service (see Appendix A)
2. **Systemd Services**: Different unit files launching same binary (see Appendix B)
3. **Version Management**: Single version number covers both services
4. **Debugging**: Same binary means consistent symbol tables across services
5. **Rollback**: Rolling back one service means rolling back both

## Implementation

1. Create cargo workspace with tk-core library crate
2. Move shared database logic to tk-core (sqlx connection, migrations)
3. Implement subcommand parsing in main.rs using clap derive macros
4. Each subcommand initializes appropriate service (gRPC or HTTP)
5. Embed migrations in tk-core via `sqlx::migrate!("./migrations")`
6. Both services call `tk_core::db::run_migrations()` on startup

## Related Decisions

This ADR implements the Simplicity principle from ADR-001 and extends the two-service architecture from ADR-006 with a unified deployment strategy.

**Depends on:**
- **ADR-001: Architectural Principles** - Implements the Simplicity principle through unified binary distribution
- **ADR-006: Service Architecture** - Extends the two-service architecture with deployment strategy

**Extended by:**
- **ADR-007: CLI Configuration** - Uses clap for subcommand implementation
- **ADR-010: Database Migrations** - Shared migration logic between services

## Alternatives Considered

### Separate Binaries

**Approach**: Build `tk-sensor-api` and `tk-web-ui` as independent binaries.

**Pros**:
- Smaller individual binaries
- True independent deployment
- Can use different dependency versions

**Cons**:
- Code duplication (database logic, migrations, models)
- Version skew risk between services
- More complex release process (two artifacts)
- Larger combined deployment size
- Manual coordination of shared logic updates

**Rejected**: Violates ADR-001 Simplicity principle. Five-engineer team cannot maintain duplicate code across services.

### Monolithic Service

**Approach**: Single service handling both gRPC and HTTP protocols.

**Pros**:
- Simplest deployment (one service, one process)
- Truly shared memory and resources
- No version coordination needed

**Cons**:
- Violates separation of concerns (ADR-006)
- Cannot scale services independently
- Mixed protocol handling increases complexity
- Single failure point affects both interfaces

**Rejected**: ADR-006 explicitly separates sensor communication (machine protocol) from human interface (HTTP). Conflating these would complicate both.

### Shared Library + Separate Binaries

**Approach**: `tk-core` as shared library, `tk-sensor-api` and `tk-web-ui` as thin binary wrappers.

**Pros**:
- Clear code separation
- Smaller individual binaries
- Can deploy services independently

**Cons**:
- More complex build process (library + two binaries)
- Version coordination still required (library vs binary versions)
- Deployment tracks three artifacts instead of one
- No operational benefit over subcommands

**Rejected**: Adds complexity without addressing core operational concerns. Subcommands provide same flexibility with simpler deployment.

## Future Considerations

- **Service Discovery**: If horizontal scaling requires service mesh, subcommand model still works (different containers, same binary)
- **Plugin Architecture**: Could add subcommands dynamically if needed
- **Multi-Binary Mode**: Could provide separate binaries as build option without changing code structure
- **WebAssembly**: Workspace structure enables compiling services to WASM independently

## Appendix A: Cargo Workspace Structure

```
trapperkeeper/
├── Cargo.toml           # Workspace definition
├── tk-core/             # Shared library crate
│   ├── src/
│   │   ├── db/          # Database access, migrations
│   │   ├── auth/        # Authentication primitives
│   │   ├── config/      # Configuration types
│   │   └── models/      # Shared data models
│   └── Cargo.toml
└── trapperkeeper/       # Main binary crate
    ├── src/
    │   ├── main.rs      # CLI with subcommands
    │   ├── sensor_api/  # gRPC service implementation
    │   └── web_ui/      # HTTP service implementation
    └── Cargo.toml
```

## Appendix B: Service Execution Examples

**Command-line usage**:
```bash
# Start sensor API
trapperkeeper sensor-api --port 50051 --db-url sqlite:trapperkeeper.db

# Start web UI
trapperkeeper web-ui --port 8080 --db-url sqlite:trapperkeeper.db

# Run migrations
trapperkeeper migrate --db-url sqlite:trapperkeeper.db
```

**Container deployment**:
```dockerfile
FROM debian:bookworm-slim
COPY trapperkeeper /usr/local/bin/
# sensor-api container
CMD ["trapperkeeper", "sensor-api", "--port", "50051"]
```

**Systemd service**:
```ini
[Service]
ExecStart=/usr/local/bin/trapperkeeper sensor-api --port 50051
```
