# Cross-Cutting Index Test Fixtures

Test cases for validating cross-cutting index documents against template requirements.

## Valid Test Cases

### valid-security.md

Complete security index meeting all requirements:

- All required frontmatter fields present
- cross_cutting_concern is "security" (valid enum value)
- Has Overview section with 2 paragraphs (within 1-3 range)
- Has Consolidated Documents section
- Proper date format (YYYY-MM-DD)

### valid-performance.md

Complete performance index meeting all requirements:

- All required frontmatter fields present
- cross_cutting_concern is "performance" (valid enum value)
- Has Overview section with 1 paragraph (within 1-3 range)
- Has Consolidated Documents section

## Invalid Test Cases

### invalid-frontmatter.md

Missing required frontmatter fields:

- MISSING: cross_cutting_concern
- Should fail frontmatter validation

### invalid-concern.md

Invalid cross_cutting_concern value:

- cross_cutting_concern: "testing-quality" (not in allowed enum)
- Should fail field constraint validation
- Valid values: security, performance, validation, observability, error-handling

## Validation Commands

```bash
# Validate all fixtures
python doc/scripts/validate.py doc/scripts/fixtures/cross-cutting-index/*.md

# Validate specific test case
python doc/scripts/validate.py doc/scripts/fixtures/cross-cutting-index/valid-security.md
```

## Expected Results

- valid-security.md: PASS
- valid-performance.md: PASS
- invalid-frontmatter.md: FAIL (missing required fields)
- invalid-concern.md: FAIL (invalid enum value)
