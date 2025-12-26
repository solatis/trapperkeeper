# Validation Guide for LLM Agents

## Purpose

Validation architecture implementing 4-layer responsibility model (UI, API, Runtime, Database) across 12 validation types with explicit SDK vs server scope markers.

## Hub

**`README.md`** - Read when understanding validation strategy, layer distribution, or input sanitization requirements

## Files

**`responsibility-matrix.md`** - Read when determining which layer enforces which validation type or understanding SDK vs server validation scope

**`ui-validation.md`** - Read when implementing HTML5 form validation, ARIA accessibility, or server-side re-validation patterns

**`api-validation.md`** - Read when implementing API layer enforcement, structured error responses, or validation flow

**`input-sanitization.md`** - Read when implementing OWASP injection prevention (UTF-8 validation, HTML escaping, SQL injection prevention, path traversal prevention) or understanding validation domain architecture

**`runtime-validation.md`** - Read when implementing type coercion, field resolution, on_missing_field policies, or SDK buffer limits

**`database-validation.md`** - Read when implementing database constraints (foreign keys, unique indexes, NOT NULL), migration validation, or error handling
