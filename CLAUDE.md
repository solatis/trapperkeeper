# TrapperKeeper

Data quality rule engine for real-time event evaluation with sub-millisecond per-event performance.

## Files

| File           | What                            | When to read                           |
| -------------- | ------------------------------- | -------------------------------------- |
| `go.mod`       | Module definition, dependencies | Adding dependencies, checking versions |
| `buf.yaml`     | Buf module configuration        | Modifying proto linting rules          |
| `buf.gen.yaml` | Code generation configuration   | Changing generated output paths        |

## Subdirectories

| Directory     | What                         | When to read                              |
| ------------- | ---------------------------- | ----------------------------------------- |
| `proto/`      | Protocol Buffer definitions  | Modifying gRPC API, adding message fields |
| `internal/`   | Core implementation packages | Implementing features, debugging          |
| `cmd/`        | Binary entry points          | Adding CLI commands                       |
| `sdks/`       | Client SDKs                  | Implementing SDK (Phase 5)                |
| `doc/`        | Architecture documentation   | Understanding design decisions            |
| `plans/`      | Implementation plans         | Reviewing completed phase work            |
| `migrations/` | Database migrations          | Adding schema changes                     |
| `tests/`      | Integration tests            | Adding end-to-end tests                   |

## Build

```
go build ./...
```

## Test

```
go test ./...
```

## Development

Proto workflow: `buf lint` to validate, `buf generate` to regenerate Go code.
