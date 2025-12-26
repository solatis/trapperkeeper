# Redirect Stub Test Fixtures

Test cases for validating redirect stub documents against template requirements.

## Valid Test Cases

### valid-stub.md
Complete redirect stub meeting all requirements:
- All required frontmatter fields present (doc_type, status, superseded_by)
- status is "superseded" (required value)
- superseded_by ends with .md (matches pattern)
- Total lines: 13 (under 15-line limit)
- Has "Redirect Notice" section
- Section content matches pattern "This document has been.*superseded"

## Invalid Test Cases

### invalid-too-long.md
Exceeds maximum line limit:
- Total lines: 26 (exceeds 15-line max_lines constraint)
- Should fail max_lines validation
- All other constraints satisfied

### invalid-missing-notice.md
Missing required "Redirect Notice" section:
- Has all required frontmatter
- Under line limit
- MISSING: "Redirect Notice" heading
- Should fail required_sections validation

## Validation Commands

```bash
# Validate all fixtures
python doc/scripts/validate.py doc/scripts/fixtures/redirect-stub/*.md

# Validate specific test case
python doc/scripts/validate.py doc/scripts/fixtures/redirect-stub/valid-stub.md
```

## Expected Results

- valid-stub.md: PASS
- invalid-too-long.md: FAIL (exceeds max_lines)
- invalid-missing-notice.md: FAIL (missing required section)

## Redirect Stub Characteristics

Redirect stubs are minimal documents that:
1. Must be very short (15 lines maximum)
2. Always have status "superseded"
3. Must point to new location with .md extension
4. Must have "Redirect Notice" section with specific content pattern
5. Should not contain implementation details or significant content
