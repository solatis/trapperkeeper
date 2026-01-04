# internal/

## Files

| File        | What                                | When to read                           |
| ----------- | ----------------------------------- | -------------------------------------- |
| `README.md` | Package architecture and boundaries | Understanding dependency graph, design |

## Subdirectories

| Directory   | What                                | When to read                                   |
| ----------- | ----------------------------------- | ---------------------------------------------- |
| `types/`    | Domain models, errors, ID helpers   | Adding validation logic, understanding limits  |
| `rules/`    | Rule compilation and evaluation     | Implementing rule engine, debugging evaluation |
| `protobuf/` | Generated Go code from protos       | Never edit; regenerate with `buf generate`     |
| `core/`     | Server-side code (db, auth, config) | Implementing API service (Phase 4)             |
