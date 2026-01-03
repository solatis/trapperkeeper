---
doc_type: spoke
status: active
date_created: 2025-11-10
primary_category: validation
hub_document: doc/07-validation/README.md
tags:
  - validation
  - responsibility-matrix
  - architecture
---

# Validation Responsibility Matrix

## Context

This matrix consolidates validation layer responsibilities for all 12 validation types across the four enforcement layers (UI, API, Runtime, Database). Each validation type specifies which layer enforces which checks, with explicit SDK vs Server scope markers for Runtime layer validations.

**Hub Document**: This document is part of the Validation Architecture. See [Validation Hub](README.md) for complete validation strategy and cross-references to detailed implementation patterns.

## Legend

- **✓** = Layer enforces this validation
- **✗** = Layer does not enforce this validation
- **SDK** = SDK-side runtime enforcement (tk-client)
- **SERVER** = Server-side runtime enforcement (tk-core)
- **BOTH** = Enforced in both SDK and server runtime

## Responsibility Matrix

| Validation Type                     | UI Layer                                             | API Layer                                                      | Runtime Layer                                                     | Database Layer                       |
| ----------------------------------- | ---------------------------------------------------- | -------------------------------------------------------------- | ----------------------------------------------------------------- | ------------------------------------ |
| **1. Structural Format Validation** | ✓ HTML5 patterns (UUID, email, URL)                  | ✓ Primary enforcement (UUID, timestamp, field path)            | ✓ **BOTH** Deserialization validation                             | ✓ Type constraints (TEXT, INTEGER)   |
| **2. Rule Expression Validation**   | ✓ Operator/field_type UI controls, sample_rate range | ✓ Expression syntax, compatibility, wildcards, pre-compilation | ✓ **BOTH** Runtime constraint enforcement                         | ✗ No CHECK constraints               |
| **3. Type Coercion**                | ✗ Forms submit strings                               | ✓ Validate parseability, detect mismatches                     | ✓ **BOTH** Coercion per type matrix; null vs coercion distinction | ✓ Column type constraints            |
| **4. Resource Limits**              | ✓ maxlength on forms (tag 128 chars)                 | ✓ Metadata (64 pairs, 64KB), tag length, depth limits          | ✓ **SDK ONLY** Event buffer (128 count, 1MB/event, 128MB cap)     | ✗ Application enforces               |
| **5. Configuration & Startup**      | ✗ No UI for config                                   | ✗ Validated at startup                                         | ✓ **SERVER ONLY** Config types, dependencies, DB connection       | ✓ Connection validation              |
| **6. Web Form Input**               | ✓ HTML5 + go-playground/validator, CSRF              | ✓ Length, email, status codes (422 vs 400)                     | ✗ N/A                                                             | ✗ N/A                                |
| **7. Authentication**               | ✓ Password validation, session cookies               | ✓ bcrypt, API key format, HMAC-SHA256, expiry                  | ✓ **SERVER ONLY** Token/signature verification                    | ✓ Unique constraints, NOT NULL       |
| **8. TLS & Transport**              | ✗ Reverse proxy enforces                             | ✓ Certificate, timestamp drift (<100ms)                        | ✓ **BOTH** Cert validation (SDK gRPC, server mTLS)                | ✗ External config                    |
| **9. Data Integrity**               | ✗ N/A                                                | ✗ Startup only                                                 | ✓ **SERVER ONLY** Migration checksums (SHA256)                    | ✓ Foreign keys, unique indexes       |
| **10. Field Resolution**            | ✗ N/A                                                | ✓ Field path syntax pre-validation                             | ✓ **BOTH** Missing field, on_missing_field policy                 | ✗ Runtime only                       |
| **11. Performance & Cost**          | ✗ No cost UI                                         | ✓ Cost budget (<1ms target), reject exceeding                  | ✓ **BOTH** Sampling rate (0.0-1.0) enforcement                    | ✗ Stored as INTEGER                  |
| **12. Input Sanitization** 🔒       | ✓ html/template auto-escaping, CSRF                  | ✓ **PRIMARY** UTF-8, HTML, SQL, path traversal                 | ✓ **BOTH** Defense-in-depth UTF-8 re-validation                   | ✓ database/sql parameterized queries |

**Cross-References by Validation Type**:

1. UUID Strategy, Timestamp Representation, Rule Expression Language
2. Rule Expression Language, Type System, Field Path Resolution
3. Type System, Schema Evolution, Field Path Resolution
4. SDK Model, Client Metadata, Event Schema
5. Configuration Management, CLI Configuration, Database Backend
6. UI Validation, API Validation, Web Framework
7. Authentication (Web UI), Authentication (Sensor API), TLS/HTTPS Strategy
8. TLS/HTTPS Strategy, Encryption Strategy, Configuration Security
9. Database Migrations, Database Validation
10. Field Path Resolution, Schema Evolution, Type System
11. Performance Model, Cost Model, Sampling Strategy
12. API Validation, Security Hub, Web Framework, Database Backend

---

## Validation Flow Summary

### Request Processing Flow

```
1. UI Layer (Browser)
   ↓ HTML5 validation
   ↓ Submit to server

2. API Layer (Primary Enforcement)
   ↓ UTF-8 + control character validation
   ↓ Structural format validation (UUIDs, timestamps, paths)
   ↓ Input sanitization (SECURITY-CRITICAL)
   ↓ Business logic validation (operator compatibility, resource limits, cost budget)
   ↓ Pre-compile rule expression
   ↓ Persist to database

3. Runtime Layer (Evaluation)
   ↓ SDK/Server: Load validated rules
   ↓ SDK/Server: Type coercion during evaluation
   ↓ SDK/Server: Field resolution with on_missing_field policy
   ↓ SDK only: Buffer limit enforcement
   ↓ SDK/Server: Sampling rate enforcement

4. Database Layer (Storage Integrity)
   ↓ Type constraints
   ↓ Foreign key constraints
   ↓ Unique indexes
   ↓ NOT NULL constraints
```

### Validation Principles

1. **Validate Early in Development**: Catch errors during rule creation, not during event processing
2. **Validate Once in Production**: Pre-compile at sync time; runtime assumes valid rules
3. **API Layer as Primary Boundary**: Most validation occurs at API before expensive operations
4. **Defense in Depth**: Runtime re-validates critical security controls
5. **Fail Fast**: Invalid configuration/data fails service startup; no partial degradation

## Related Documents

**Hub**: Validation Architecture - Complete validation strategy overview

**Spokes** (layer-specific details):

- UI Validation - HTML5 validation, form patterns, ARIA accessibility
- API Validation - Input sanitization, structured errors, validation flow
- Runtime Validation - Type coercion, field resolution, SDK vs server scope
- Database Validation - Constraints, migrations, error handling

**Cross-Cutting Indexes**:

- Security Index - Security-critical validation controls
- Performance Index - Cost validation, sampling enforcement
- Error Handling Index - Validation error patterns across layers

**Dependencies**:

- Architectural Principles - Validation philosophy and centralization principle
- Rule Expression Language - Expression syntax and semantics
- Type System - Coercion rules and null handling
- Field Path Resolution - Field path syntax and wildcard semantics
- Schema Evolution - on_missing_field policy specification
