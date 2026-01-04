# types/

Domain models shared across TrapperKeeper components.

## Files

| File        | What                                     | When to read                             |
| ----------- | ---------------------------------------- | ---------------------------------------- |
| `types.go`  | EventID, RuleID, Payload, Metadata types | Working with core types, checking limits |
| `errors.go` | Sentinel errors for validation failures  | Handling errors, adding error variants   |
| `ids.go`    | UUIDv7 ID generation and parsing         | Generating or parsing identifiers        |
| `rules.go`  | Rule domain types for internal use       | Working with compiled rules              |
| `README.md` | Dependency constraints and design notes  | Understanding package boundaries         |
