# Test Fixtures for RuleEvaluator

This directory contains example files demonstrating the RuleEvaluator functionality.

## Files

- **example_template.md** - Template with validation frontmatter
- **valid_document.md** - Document that passes all validation rules
- **invalid_document.md** - Document that violates multiple rules

## Usage Example

```python
from pathlib import Path
from rule_evaluator import validate_document

# Define validation schema
validation_schema = {
    "schema_version": 1,
    "title_pattern": "^# .+ Guide$",
    "max_lines": 50,
    "forbidden": [
        {"pattern": "(?i)how to", "reason": "how-to instructions", "severity": "error"},
        {"pattern": "(?i)step 1", "reason": "step-by-step procedures", "severity": "error"}
    ],
    "filename_pattern": "^[a-z_-]+\\.md$"
}

# Validate document
doc_path = Path("valid_document.md")
errors = validate_document(doc_path, validation_schema)

if errors:
    print(f"Validation failed with {len(errors)} error(s):")
    for error in errors:
        print(error.format_error())
else:
    print("Validation passed!")
```

## Supported Rules (M4)

1. **title_pattern** - Regex match on first H1 heading
2. **max_lines** - Line count limit
3. **forbidden** - List of forbidden patterns with reasons
4. **filename_pattern** - Regex match on filename

## Future Rules (M5+)

- Conditional validation based on document frontmatter
- Section structure validation
- File listing validation
- Subsection count validation
