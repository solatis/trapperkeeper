---
doc_type: template
validation:
  schema_version: 1
  title_pattern: "^# .+ Guide$"
  max_lines: 50
  forbidden:
    - pattern: "(?i)how to"
      reason: "how-to instructions"
      severity: error
    - pattern: "(?i)step 1"
      reason: "step-by-step procedures"
      severity: error
  filename_pattern: "^[a-z_-]+\\.md$"
---

# Example Template for Testing

This template demonstrates the RuleEvaluator integration with validation frontmatter.

## Purpose

Templates define validation rules that documents must follow.

## Example Rules

The validation block above defines:

- Title must end with "Guide"
- Maximum 50 lines
- No forbidden patterns (how-to, step-by-step)
- Filename must be lowercase with dashes/underscores
