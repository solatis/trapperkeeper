# TrapperKeeper Codebase Guide for LLM Agents

## Purpose

Data quality rule engine for real-time event evaluation. Schema-agnostic design
with sub-millisecond per-event performance target.

## Quick Navigation

| Directory            | Purpose                               | Read When                                           |
| -------------------- | ------------------------------------- | --------------------------------------------------- |
| `proto/`             | Protocol Buffer definitions           | Modifying gRPC API, adding fields to messages       |
| `internal/protobuf/` | Generated Go code from protos         | Never edit directly; regenerate with `buf generate` |
| `internal/types/`    | Domain models, error types, constants | Adding validation logic, understanding limits       |
| `internal/rules/`    | Rule compilation and evaluation       | Implementing rule engine (Phase 2)                  |
| `internal/core/`     | Server-side code (db, auth, config)   | Implementing API service (Phase 4)                  |
| `sdks/go/`           | Go SDK for sensors                    | Implementing SDK (Phase 5)                          |
| `cmd/trapperkeeper/` | Binary entry points                   | Adding CLI commands                                 |
| `doc/`               | Architecture documentation            | Understanding design decisions                      |

## Key Files

| File           | Contents                        | Read When                              |
| -------------- | ------------------------------- | -------------------------------------- |
| `go.mod`       | Module definition, dependencies | Adding dependencies, checking versions |
| `buf.yaml`     | Buf module configuration        | Modifying proto linting rules          |
| `buf.gen.yaml` | Code generation configuration   | Changing generated output paths        |

## Commands

| Command          | Purpose                        |
| ---------------- | ------------------------------ |
| `buf lint`       | Validate proto files           |
| `buf generate`   | Regenerate Go code from protos |
| `go build ./...` | Build all packages             |
| `go test ./...`  | Run all tests                  |
