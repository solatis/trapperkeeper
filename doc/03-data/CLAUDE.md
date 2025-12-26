# Data Architecture Guide for LLM Agents

## Purpose

Event storage model (JSONL files), UUIDv7 identifiers, timestamp handling across boundaries, and metadata namespace management for Go implementation.

## Hub

**`README.md`** - Read when understanding data architecture, event storage strategy, or schema-agnostic design

## Files

**`event-schema-storage.md`** - Read when implementing JSONL event storage, understanding audit trail preservation, or database schema design

**`identifiers-uuidv7.md`** - Read when implementing UUIDv7 generation (github.com/google/uuid), understanding time-ordering properties, or identifier strategy

**`timestamps.md`** - Read when implementing time.Time handling across SDK/gRPC/database boundaries, understanding timezone policies, or timestamp validation

**`metadata-namespace.md`** - Read when implementing metadata field handling, understanding reserved namespace (client.\*), or field path resolution with metadata
