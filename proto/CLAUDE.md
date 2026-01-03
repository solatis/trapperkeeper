# Proto Directory Guide for LLM Agents

## Purpose

Protocol Buffer definitions for TrapperKeeper gRPC API.

## Files

| File                                       | Contents                 | Read When                 |
| ------------------------------------------ | ------------------------ | ------------------------- |
| `trapperkeeper/sensor/v1/sensor_api.proto` | Service definition, RPCs | Modifying API endpoints   |
| `trapperkeeper/sensor/v1/rule.proto`       | Rule, Condition, enums   | Changing rule structure   |
| `trapperkeeper/sensor/v1/field_path.proto` | FieldPath, PathSegment   | Modifying path resolution |
| `trapperkeeper/sensor/v1/event.proto`      | Event message            | Changing event structure  |

## After Editing

1. Run `buf lint` to validate changes
2. Run `buf generate` to regenerate Go code
3. Run `go build ./internal/protobuf/...` to verify compilation
