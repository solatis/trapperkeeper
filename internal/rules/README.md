# Rules Package

## Overview

Rule compilation and evaluation engine implementing DNF rule structure, JSON
field path resolution with wildcards, type coercion, and sub-millisecond
per-event evaluation.

## Architecture

Evaluation flow: Compile() -> CompiledRule -> Evaluate(payload) -> MatchResult

Field resolution uses PathSegment traversal with wildcard support for ANY
semantics (first match wins).

## Design Decisions

Cost-based condition ordering: Conditions within AND groups are automatically
sorted by ascending cost to fail fast on cheap checks.

Null handling strategy: Null values defer to on_missing_field configuration;
coercion failures use on_coercion_fail. This separation allows different
policies for missing vs malformed data.

## Invariants

- Maximum 2 nested wildcards per path
- Maximum 16 path depth
- Maximum 64 values in IN operator
- Wildcards resolve to ANY semantics (first match)

## Dependencies

- internal/types: Domain models (RuleID, sentinel errors)
- encoding/json: Payload parsing
- No protobuf imports (conversion happens at boundary layer)
