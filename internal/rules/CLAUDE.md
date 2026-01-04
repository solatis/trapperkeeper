# rules/

Rule compilation and evaluation engine for TrapperKeeper.

## Files

| File           | What                                  | When to read                                      |
| -------------- | ------------------------------------- | ------------------------------------------------- |
| `fieldpath.go` | JSON field path resolution, wildcards | Debugging field resolution, adding path operators |
| `coercion.go`  | Type coercion, FieldType enum         | Fixing type coercion bugs, adding field types     |
| `cost.go`      | Cost constants, condition ordering    | Tuning performance, updating cost model           |
| `compile.go`   | Rule compilation and validation       | Adding rule validation, changing compilation      |
| `operators.go` | Comparison operator implementations   | Adding operators, fixing comparison bugs          |
| `evaluate.go`  | Rule evaluation, MatchResult          | Debugging evaluation, adding match diagnostics    |
| `*_test.go`    | Unit tests for each module            | Understanding expected behavior, adding tests     |
| `README.md`    | Design invariants and architecture    | Understanding rule engine design decisions        |
