# Rules Package Guide for LLM Agents

## Purpose

Rule compilation and evaluation engine for TrapperKeeper. Implements DNF rule
structure, JSON field path resolution with wildcards, type coercion, and
sub-millisecond per-event evaluation.

## Files

| File           | Contents                                   | Read When                                         |
| -------------- | ------------------------------------------ | ------------------------------------------------- |
| `fieldpath.go` | Resolve(), PathSegment, wildcard traversal | Debugging field resolution, adding path operators |
| `coercion.go`  | Coerce(), FieldType, null vs error logic   | Fixing type coercion bugs, adding field types     |
| `cost.go`      | Cost constants, CalculateConditionCost     | Tuning performance, updating cost model           |
| `compile.go`   | Compile(), CompiledRule, validation        | Adding rule validation, changing compilation      |
| `operators.go` | Compare(), operator implementations        | Adding operators, fixing comparison bugs          |
| `evaluate.go`  | Evaluate(), MatchResult, sample rate       | Debugging evaluation, adding match diagnostics    |

## Key Invariants

1. Null values defer to on_missing_field; coercion failures use on_coercion_fail
2. Conditions within AND groups ordered by ascending cost
3. Wildcards resolve to ANY semantics (first match)
4. Maximum 2 nested wildcards, 16 path depth, 64 IN values

## Dependencies

- internal/types: Domain models (RuleID, sentinel errors)
- encoding/json: Payload parsing
- No protobuf imports (conversion at boundary layer)
