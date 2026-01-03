# Types Package Guide for LLM Agents

## Purpose

Domain models shared across TrapperKeeper components. Core types have zero
external dependencies; ID utilities have uuid dependency.

## Files

| File        | Contents                                                       | Read When                                |
| ----------- | -------------------------------------------------------------- | ---------------------------------------- |
| `types.go`  | EventID, RuleID (string aliases), Payload, Metadata, constants | Working with types, checking limits      |
| `errors.go` | Sentinel errors for validation failures                        | Handling errors, adding new error types  |
| `ids.go`    | NewEventID, NewRuleID, ParseEventID, ParseRuleID, EventIDTime  | Generating or parsing UUIDv7 identifiers |

## Constraints

- **types.go, errors.go**: Only stdlib (encoding/json) allowed
- **ids.go**: May import github.com/google/uuid for ID generation
- **No proto imports**: Proto types live in internal/protobuf
- **Thin types**: Minimal logic; validation in internal/rules
