---
doc_type: spoke
status: active
date_created: 2025-11-07
primary_category: validation
hub_document: doc/07-validation/README.md
tags:
  - ui-validation
  - forms
  - accessibility
---

# UI Layer Validation

## Context

UI layer validation provides first line of defense for user input, focusing on immediate feedback and prevention of invalid combinations. Server-side rendering with HTML5 validation provides security and accessibility while maintaining simplicity principles (minimal JavaScript).

**Hub Document**: This document is part of the Validation Architecture. See [Validation Hub](README.md) for complete validation strategy and layer distribution.

## HTML5 Form Validation

Browser-side validation before submission using HTML5 attributes.

### Form Attributes

**Required fields**:

```html
<input type="text" name="rule_name" required />
```

**Length constraints**:

```html
<input type="text" name="rule_name" maxlength="128" />
```

**Pattern validation**:

```html
<input
  type="text"
  name="uuid"
  pattern="[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}"
/>
```

**Type validation**:

```html
<input type="email" name="email" />
<input type="number" name="priority" min="0" max="100000" />
<input type="url" name="webhook_url" />
```

**Cross-References**:

- Rule Expression Language: Rule name length limits
- UUID Strategy: UUIDv7 format specification

## Server-Side Form Validation

Server-side re-validation using go-playground/validator ensures security even if client-side checks bypassed.

### go-playground/validator Integration

```go
import (
    "github.com/go-playground/validator/v10"
)

type RuleForm struct {
    Name         string   `validate:"required,min=1,max=128"`
    SampleRate   float64  `validate:"min=0.0,max=1.0"`
    ContactEmail *string  `validate:"omitempty,email"`
}

// Validate on form submission
validate := validator.New()
if err := validate.Struct(form); err != nil {
    renderErrors(err)
    return
}
processForm(form)
```

**Rationale**: Defense in depth - server-side validation protects against malicious clients, disabled JavaScript, and browser bugs.

**Cross-References**:

- API Validation: Structured error response format

## Inline Error Messages with ARIA

Accessibility-focused error messages adjacent to form fields.

### Error Display Pattern

```html
<div class="form-field">
  <label for="rule-name">Rule Name</label>
  <input
    id="rule-name"
    name="name"
    aria-invalid="true"
    aria-describedby="name-error"
    value="{{ submitted_value }}"
  />
  <span id="name-error" class="error">
    Rule name must be 1-128 characters
  </span>
</div>
```

**Error Summary**:

```html
<div class="error-summary" role="alert">
  <h2>Please correct the following errors:</h2>
  <ul>
    <li><a href="#name-error">Rule name must be 1-128 characters</a></li>
    <li><a href="#rate-error">Sample rate must be between 0.0 and 1.0</a></li>
  </ul>
</div>
```

**Rationale**: ARIA attributes enable screen reader navigation. Error summary provides overview before form. Links allow keyboard navigation to fields.

**Cross-References**:

- Web Framework: html/template for error rendering

## Prevention Through UI Controls

Prevent invalid combinations through UI design rather than error messages.

### Operator/Field Type Compatibility

```html
<!-- When user selects 'gt' operator, field_type dropdown shows only compatible types -->
<select name="operator" onchange="filterFieldTypes(this.value)">
  <option value="eq">equals</option>
  <option value="gt">greater than</option>
  <!-- ... -->
</select>

<select name="field_type" id="field_type_select">
  <option value="any">any</option>
  <option value="numeric" data-compatible="gt,gte,lt,lte,eq,neq">
    numeric
  </option>
  <option value="text" data-compatible="eq,neq,prefix,suffix,contains">
    text
  </option>
</select>
```

**Note**: Server-side rendering minimizes JavaScript. Dynamic UI updates are future consideration requiring JavaScript for real-time interaction.

**Cross-References**:

- Rule Expression Language: Operator/field_type compatibility matrix

## CSRF Protection

CSRF token validation for all state-changing operations.

### Token Inclusion

```html
<form method="POST" action="/rules">
  <input type="hidden" name="csrf_token" value="{{ csrf_token }}" />
  <!-- form fields -->
</form>
```

**Verification**: Server validates token before processing POST/PUT/DELETE requests.

**Cross-References**:

- Web Framework: CSRF protection configuration
- Authentication: Session-based CSRF tokens

## Form Preservation on Error

Preserve user input when validation fails to improve user experience.

### Input Preservation Pattern

```html
<input
  type="text"
  name="rule_name"
  value="{{ submitted_value | default(value='') }}"
  aria-invalid="{% if errors.rule_name %}true{% endif %}"
/>
```

**Rationale**: Users don't lose work when validation fails. Reduces frustration and abandonment.

## Related Documents

**Dependencies** (read these first):

- Web Framework: html/template, CSRF protection
- Validation Hub: Complete validation strategy

**Related Spokes** (siblings in this hub):

- Responsibility Matrix: Complete UI Layer validation assignments for all 12 validation types
- API Validation: Server-side validation enforces UI validation
- Runtime Validation: Runtime validation complements UI validation

**Extended by** (documents building on this):

- Authentication: Login form validation patterns
