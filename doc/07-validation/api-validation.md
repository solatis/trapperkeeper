---
doc_type: spoke
status: active
date_created: 2025-11-07
primary_category: validation
hub_document: doc/07-validation/README.md
tags:
  - api-validation
  - input-sanitization
  - security
---

# API Layer Validation

## Context

API layer serves as primary enforcement point for all validation rules. Early validation before expensive operations (database writes, rule compilation) improves performance and provides clear error feedback. Input sanitization at API boundary prevents injection attacks and ensures data integrity.

**Hub Document**: This document is part of the Validation Architecture. See [Validation Hub](README.md) for complete validation strategy and layer distribution.

## Input Sanitization (SECURITY-CRITICAL)

Five core security controls enforced at API boundary following OWASP guidelines. See Input Sanitization for complete specifications with Go implementation examples, validation domain architecture clarifications, attack examples, and OWASP compliance checklist.

**Five Core Security Controls**:

1. **UTF-8 Validation**: Reject control characters 0x00-0x1F (except tab/LF/CR)
2. **HTML Escaping**: Use html/template (automatic context-appropriate escaping)
3. **SQL Injection Prevention**: Use database/sql parameterized queries EXCLUSIVELY (no string concatenation)
4. **Command Injection Prevention**: Never spawn processes (architectural constraint)
5. **Path Traversal Prevention**: Canonicalize paths and validate within data directory

**API Layer Enforcement**: Primary security boundary for all sanitization controls with structured error responses (422 for validation failures).

**Cross-References**:

- Input Sanitization: Complete OWASP sanitization patterns and implementation details
- Runtime Validation: Defense-in-depth re-validation patterns
- Web Framework: html/template
- Database Backend: database/sql parameterized queries

## Structured Error Responses

HTTP status codes with JSON error details.

### Status Codes

- **422 Unprocessable Entity**: Validation errors (well-formed request, invalid data)
- **400 Bad Request**: Malformed requests (invalid JSON, missing required fields)
- **401 Unauthorized**: Authentication failures
- **500 Internal Server Error**: Unexpected server errors (never expose internal details)

### Response Format

```json
{
  "error": "Validation failed",
  "code": "VALIDATION_ERROR",
  "request_id": "01936a3e-1234-7b3c-9d5e-abcdef123456",
  "details": [
    {
      "field": "sample_rate",
      "message": "sample_rate must be between 0.0 and 1.0",
      "code": "OUT_OF_RANGE"
    }
  ]
}
```

**Never Expose**: Internal database error messages, stack traces in production, system paths, SQL query text.

## Validation Flow

Pre-compilation at sync time prevents runtime errors.

### Validation Order

1. Validate UTF-8 encoding and control characters → 422 if invalid
2. Validate JSON structure → 400 if malformed
3. Validate UUIDs, timestamps, field paths → 422 if invalid format
4. Validate operator/field_type compatibility → 422 if invalid combination
5. Validate cost budget, sample_rate range → 422 if out of range
6. Pre-compile rule expression → 422 if compilation fails
7. Write to database → 500 if database error
8. Return 201 Created with rule resource

**Rationale**: Early validation rejects invalid requests before expensive operations. Clear error messages before partial processing. Prevents cascading failures from invalid data.

## Related Documents

**Dependencies**: Validation Hub, Web Framework, Database Backend

**Related Spokes**:

- Responsibility Matrix: Complete API Layer validation assignments for all 12 validation types (primary enforcement point)
- UI Validation: Client-side validation provides first line of defense
- Runtime Validation: Runtime validation complements API validation
- Database Validation: Database constraints provide final enforcement
