# proto/

Protocol Buffer definitions for TrapperKeeper gRPC API.

## Subdirectories

| Directory        | What                 | When to read                   |
| ---------------- | -------------------- | ------------------------------ |
| `trapperkeeper/` | Sensor API v1 protos | Modifying API, adding messages |

## Files

| File                                       | What                     | When to read              |
| ------------------------------------------ | ------------------------ | ------------------------- |
| `trapperkeeper/sensor/v1/sensor_api.proto` | Service definition, RPCs | Modifying API endpoints   |
| `trapperkeeper/sensor/v1/rule.proto`       | Rule, Condition, enums   | Changing rule structure   |
| `trapperkeeper/sensor/v1/field_path.proto` | FieldPath, PathSegment   | Modifying path resolution |
| `trapperkeeper/sensor/v1/event.proto`      | Event message            | Changing event structure  |

## Workflow

1. Edit proto files
2. Run `buf lint` to validate
3. Run `buf generate` to regenerate Go code
4. Run `go build ./internal/protobuf/...` to verify
