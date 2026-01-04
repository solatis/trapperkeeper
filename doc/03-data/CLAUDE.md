# 03-data/

Event storage model, UUIDv7 identifiers, timestamps, and metadata namespace
management.

## Files

| File                       | What                    | When to read                                 |
| -------------------------- | ----------------------- | -------------------------------------------- |
| `README.md`                | Data architecture hub   | Understanding event storage, schema-agnostic |
| `event-schema-storage.md`  | JSONL event storage     | Implementing audit trails, database schema   |
| `identifiers-uuidv7.md`    | UUIDv7 generation       | Implementing time-ordered identifiers        |
| `timestamps.md`            | Timestamp storage       | Implementing time across SDK/gRPC/database   |
| `timezone-presentation.md` | Timezone display        | Converting UTC to user-local time in web UI  |
| `metadata-namespace.md`    | Metadata field handling | Implementing reserved namespace (client.\*)  |
