# internal/core/db

Database layer supporting SQLite (development) and PostgreSQL (production).

## Files

| File            | What                                      | When to read                                 |
| --------------- | ----------------------------------------- | -------------------------------------------- |
| `db.go`         | Connection pooling, driver detection      | Modifying database connection configuration  |
| `migrations.go` | Migration runner with checksum validation | Adding migrations, debugging schema issues   |
| `queries.go`    | Named query loader using dotsql           | Adding named queries, debugging query lookup |

## Subdirectories

| Directory  | What                              | When to read                          |
| ---------- | --------------------------------- | ------------------------------------- |
| `queries/` | Named SQL files (CRUD operations) | Adding entity queries, optimizing SQL |

## External Dependencies

| File                        | What                           | When to read               |
| --------------------------- | ------------------------------ | -------------------------- |
| `migrations/migrations.go`  | Embedded migration filesystems | Modifying embed directives |
| `migrations/sqlite/*.sql`   | SQLite schema migrations       | Changing SQLite schema     |
| `migrations/postgres/*.sql` | PostgreSQL schema migrations   | Changing PostgreSQL schema |
