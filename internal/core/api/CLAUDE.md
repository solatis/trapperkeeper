# internal/core/api/

gRPC service handlers for SensorAPI (SyncRules, ReportEvents).

## Index

| File                 | Contents (WHAT)                                        | Read When (WHEN)                                       |
| -------------------- | ------------------------------------------------------ | ------------------------------------------------------ |
| `service.go`         | SensorAPIService struct, JSONL mutex management        | Adding new RPC methods, debugging service lifecycle    |
| `sync_rules.go`      | SyncRules handler, ETAG computation, rule queries      | Modifying rule sync, debugging ETAG logic              |
| `report_events.go`   | ReportEvents handler, per-event transactions, JSONL    | Debugging event ingestion, modifying validation        |
| `errors.go`          | Error mapping guidelines (inline in handlers)          | Understanding gRPC status code mapping                 |
| `README.md`          | Architecture, data flow, invariants                    | Understanding handler orchestration, troubleshooting   |
