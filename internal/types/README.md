# Types Package

## Overview

Domain models shared across TrapperKeeper components. Core types have zero
external dependencies; ID utilities have uuid dependency.

## Design Decisions

Dependency isolation by file:

- `types.go`, `errors.go`: Only stdlib (encoding/json) allowed
- `ids.go`: May import github.com/google/uuid for ID generation

## Invariants

- No proto imports: Proto types live in internal/protobuf
- Thin types: Minimal logic; validation belongs in internal/rules
