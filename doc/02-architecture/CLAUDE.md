# 02-architecture/

Two-service architecture with gRPC sensor API, HTTP web UI, unified binary, and SDK model.

## Files

| File                      | What                         | When to read                                |
| ------------------------- | ---------------------------- | ------------------------------------------- |
| `README.md`               | Architecture hub             | Understanding two-service model, boundaries |
| `service-architecture.md` | Service separation rationale | Understanding gRPC vs HTTP, deployment      |
| `api-service.md`          | gRPC sensor API design       | Implementing ETAG sync, HMAC auth           |
| `sdk-model.md`            | SDK architecture             | Implementing client-side rule evaluation    |
| `binary-distribution.md`  | Unified binary strategy      | Implementing cobra subcommands, go build    |
