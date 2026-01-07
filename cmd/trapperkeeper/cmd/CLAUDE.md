# cmd/trapperkeeper/cmd/

Cobra command implementations for TrapperKeeper CLI.

## Index

| File            | Contents (WHAT)                                         | Read When (WHEN)                                  |
| --------------- | ------------------------------------------------------- | ------------------------------------------------- |
| `root.go`       | Root command, global flags (--config, --db-url, etc.)   | Modifying global flags, adding new subcommands    |
| `sensor_api.go` | sensor-api subcommand, gRPC service initialization      | Debugging startup, modifying service config       |
