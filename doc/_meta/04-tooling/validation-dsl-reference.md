---
doc_type: reference
status: active
date_created: 2025-11-08
primary_category: tooling
hub_document: doc/_meta/04-tooling/README.md
tags:
  - validation
  - dsl
  - templates
maintainer: Documentation Team
---

# Validation DSL Reference

## Overview

The validation DSL enables template authors to define validation rules directly in template frontmatter. These rules are automatically enforced by `validate.py` when documents are created from templates, ensuring consistency and compliance with documentation standards.

**Purpose**: Define template-specific validation rules without writing custom Python code.

**Location**: Validation rules live in template frontmatter under the `validation` key.

**Execution**: `validate.py` reads validation rules from templates, matches documents to templates, and executes all applicable rules.

**Benefits**:

- Template-driven validation keeps rules adjacent to structure
- No code changes needed to add new validation constraints
- Declarative syntax easier to maintain than imperative validation code
- Automatic enforcement in CI prevents non-compliant documents from merging

## Schema Version

All validation blocks must declare `schema_version: 1` as the first field.

**Current version**: 1

**Version evolution**: Breaking changes increment version number. Non-breaking additions maintain compatibility with version 1.

**Example**:

```yaml
validation:
  schema_version: 1
  title_pattern: "^# .+ Guide$"
```

Version field ensures forward compatibility when DSL evolves. Validator checks schema version and reports errors if unsupported version encountered.

## Rule Types

### title_pattern

Validates document title (first H1 heading) matches required pattern.

**Purpose**: Enforce consistent title conventions across document types.

**Type**: String (regular expression)

**Example**:

```yaml
validation:
  schema_version: 1
  title_pattern: "^# .+ Guide for LLM Agents$"
```

**Error behavior**: Reports mismatch with expected pattern and actual title found.

**Use cases**:

- CLAUDE.md must end with "Guide for LLM Agents"
- Hub documents must end with "Architecture"
- Reference documents must end with "Reference"

### max_lines

Enforces maximum document length in lines.

**Purpose**: Keep documents concise and prevent bloat.

**Type**: Integer (minimum 1)

**Example**:

```yaml
validation:
  schema_version: 1
  max_lines: 50
```

**Error behavior**: Reports actual line count and maximum allowed.

**Use cases**:

- CLAUDE.md limited to 50 lines for fast navigation
- Hub documents capped at 500 lines to prevent monolithic files
- Template files bounded to maintain usability

### filename_pattern

Validates filename matches required pattern.

**Purpose**: Enforce naming conventions for specific document types.

**Type**: String (regular expression)

**Example**:

```yaml
validation:
  schema_version: 1
  filename_pattern: "^README\\.md$"
```

**Error behavior**: Reports incorrect filename and required pattern.

**Use cases**:

- Hub documents must be named `README.md`
- Index documents must match `*-index.md` pattern
- Templates must end with `.md` extension

### forbidden

Prohibits specified content patterns with severity levels.

**Purpose**: Detect anti-patterns and style violations.

**Type**: Array of objects with `pattern`, `reason`, and optional `severity` fields.

**Structure**:

```yaml
forbidden:
  - pattern: "regex_pattern"
    reason: "explanation of why forbidden"
    severity: error # or "warn"
```

**Severity levels**:

- `error`: Validation fails, blocks commit
- `warn`: Informational, doesn't block commit

**Example**:

```yaml
validation:
  schema_version: 1
  forbidden:
    - pattern: "(?i)how to"
      reason: "how-to instructions belong in implementation docs"
      severity: error
    - pattern: "(?i)step 1"
      reason: "step-by-step procedures violate navigation file format"
      severity: error
    - pattern: "version \\d+"
      reason: "version numbers create maintenance burden"
      severity: warn
```

**Error behavior**: Reports line number, matched pattern, and reason for prohibition.

**Best practices**:

- Use `(?i)` flag for case-insensitive matching
- Keep patterns specific to avoid false positives
- Write clear reasons explaining why pattern violates standards
- Use `warn` severity for soft guidelines

### required_sections

Validates document structure including section presence, content, and subsections.

**Purpose**: Enforce document architecture and completeness.

**Type**: Array of section requirement objects.

**Section requirement fields**:

- `name`: Section heading text (H2 level)
- `must_exist`: Boolean, section absolutely required
- `require_if`: Condition name, section required if condition true
- `forbid_if`: Condition name, section forbidden if condition true
- `min_paragraphs`: Minimum paragraph count
- `max_paragraphs`: Maximum paragraph count
- `max_sentences`: Maximum sentence count
- `content_pattern`: Regex pattern for section content
- `subsections_required`: Subsection counting rules
- `files_rules`: File listing validation

**Basic example**:

```yaml
validation:
  schema_version: 1
  required_sections:
    - name: "Purpose"
      must_exist: true
      max_paragraphs: 3
```

**Conditional example**:

```yaml
validation:
  schema_version: 1
  conditions:
    readme_exists: file_exists("README.md")
  required_sections:
    - name: "Hub"
      require_if: readme_exists
      content_pattern: '^\*\*`README\.md`\*\* - Read when'
```

**Subsections example**:

```yaml
validation:
  schema_version: 1
  required_sections:
    - name: "Decision"
      must_exist: true
      subsections_required:
        min: 3
        max: 7
        pattern: "^### "
```

**Error behavior**: Reports missing sections, paragraph count violations, content pattern mismatches, or subsection count violations.

### files_rules (within sections)

Validates file and directory listings within sections.

**Purpose**: Ensure complete inventory of files and subdirectories.

**Type**: Object with validation flags and patterns.

**Fields**:

- `must_list_all_md`: Boolean, require all .md files listed
- `must_list_all_subdirs`: Boolean, require all subdirectories listed
- `exclude_globs`: Array of glob patterns to exclude
- `entry_pattern`: Regex for entry format validation

**Example**:

```yaml
validation:
  schema_version: 1
  required_sections:
    - name: "Files"
      require_if: has_md_files
      files_rules:
        must_list_all_md: true
        exclude_globs: ["README.md", "CLAUDE.md"]
        entry_pattern: '^\*\*`.+\.md`\*\* - Read when'
```

**Error behavior**: Reports unlisted files, unlisted directories, or format violations.

**Use cases**:

- CLAUDE.md must list all documentation files
- Hub documents must reference all consolidated spokes
- Index documents must link all relevant sections

### frontmatter

Validates frontmatter fields including required fields, constraints, and conditional requirements.

**Purpose**: Ensure document metadata completeness and correctness.

**Type**: Object with field requirements and constraints.

**Sub-fields**:

#### required_fields

Array of field names that must be present.

**Example**:

```yaml
validation:
  schema_version: 1
  frontmatter:
    required_fields:
      - doc_type
      - status
      - date_created
      - primary_category
```

#### field_constraints

Per-field validation rules including enums, patterns, types, and array sizes.

**Constraint types**:

- `enum`: Allowed values (array of strings)
- `pattern`: Regex pattern for field value
- `type`: Expected type (string, integer, boolean, array, object)
- `min_items`: Minimum array length
- `max_items`: Maximum array length

**Example**:

```yaml
validation:
  schema_version: 1
  frontmatter:
    field_constraints:
      doc_type:
        enum: ["hub", "spoke", "index"]
      status:
        enum: ["draft", "active", "deprecated", "superseded"]
      date_created:
        pattern: "^\\d{4}-\\d{2}-\\d{2}$"
      consolidated_spokes:
        type: array
        min_items: 3
```

#### conditional_constraints

If-then field requirements based on other field values.

**Fields**:

- `if_field`: Field name to check
- `equals`: Value that triggers constraint
- `then_required`: Fields required when condition true
- `then_forbidden`: Fields forbidden when condition true

**Example**:

```yaml
validation:
  schema_version: 1
  frontmatter:
    conditional_constraints:
      - if_field: status
        equals: superseded
        then_required: ["superseded_by"]
      - if_field: doc_type
        equals: spoke
        then_required: ["hub_document"]
```

**Error behavior**: Reports missing required fields, enum violations, pattern mismatches, type errors, or conditional constraint failures.

## Predicates

Predicates enable conditional validation based on filesystem state or document structure.

### file_exists(path)

Checks if file exists relative to document directory.

**Parameters**: `path` - relative file path (string)

**Returns**: Boolean (true if file exists)

**Example**:

```yaml
validation:
  schema_version: 1
  conditions:
    readme_exists: file_exists("README.md")
  required_sections:
    - name: "Hub"
      require_if: readme_exists
```

**Use cases**:

- Require "Hub" section only if README.md exists
- Conditional validation based on configuration files
- Different rules for directories with/without hub documents

### md_files_exist(exclude=[])

Checks if markdown files exist in document directory after exclusions.

**Parameters**: `exclude` - array of filenames to exclude (optional)

**Returns**: Boolean (true if any non-excluded .md files found)

**Example**:

```yaml
validation:
  schema_version: 1
  conditions:
    has_md_files: md_files_exist(exclude=["README.md", "CLAUDE.md"])
  required_sections:
    - name: "Files"
      require_if: has_md_files
```

**Use cases**:

- Require "Files" section only if spoke documents exist
- Different validation for empty directories
- Conditional listing requirements

### subdirs_exist()

Checks if subdirectories exist in document directory.

**Parameters**: None

**Returns**: Boolean (true if any subdirectories found)

**Example**:

```yaml
validation:
  schema_version: 1
  conditions:
    has_subdirs: subdirs_exist()
  required_sections:
    - name: "Subdirectories"
      require_if: has_subdirs
```

**Use cases**:

- Require "Subdirectories" section only if navigation needed
- Different structure for leaf vs branch directories
- Conditional documentation inventory

### section_present(name)

Checks if section exists in target document.

**Parameters**: `name` - section heading text (string)

**Returns**: Boolean (true if section exists)

**Requirements**: Requires document AST context from markdown parser.

**Example**:

```yaml
validation:
  schema_version: 1
  conditions:
    has_examples: section_present("Examples")
  required_sections:
    - name: "Example References"
      require_if: has_examples
```

**Note**: This predicate requires parsed AST, available only during full document validation (not during template validation).

**Use cases**:

- Cross-reference validation between sections
- Conditional requirements based on content structure
- Ensuring section dependencies

## Conditional Logic

### conditions Block

Defines named predicate expressions for reuse in validation rules.

**Purpose**: Create reusable boolean expressions for `require_if` and `forbid_if` constraints.

**Structure**:

```yaml
conditions:
  condition_name: predicate_function(args)
```

**Condition names**: Must be valid identifiers (lowercase, underscores, start with letter).

**Predicate values**: Must be function calls matching predicate signatures.

**Example**:

```yaml
validation:
  schema_version: 1
  conditions:
    readme_exists: file_exists("README.md")
    has_md_files: md_files_exist(exclude=["README.md", "CLAUDE.md"])
    has_subdirs: subdirs_exist()
    has_examples: section_present("Examples")
```

**Evaluation**: Predicates evaluated once at start of validation, results cached for rule execution.

### require_if

Conditional section requirement - section required only when condition true.

**Purpose**: Make sections optional based on directory state.

**Example**:

```yaml
validation:
  schema_version: 1
  conditions:
    readme_exists: file_exists("README.md")
  required_sections:
    - name: "Hub"
      require_if: readme_exists
      content_pattern: '^\*\*`README\.md`\*\* - Read when'
```

**Behavior**:

- Condition evaluates to `true`: section becomes required
- Condition evaluates to `false`: section optional (no validation)
- Condition evaluation error: validation fails

### forbid_if

Conditional section prohibition - section forbidden when condition true.

**Purpose**: Prevent sections that don't apply in certain contexts.

**Example**:

```yaml
validation:
  schema_version: 1
  conditions:
    is_cloud_only: file_exists(".cloud-only")
  required_sections:
    - name: "Installation"
      forbid_if: is_cloud_only
```

**Behavior**:

- Condition evaluates to `true`: section must not exist
- Condition evaluates to `false`: section allowed (no restriction)
- Section present when forbidden: validation error

### Boolean Logic

**Current support**: Single condition names only.

**Syntax**: `require_if: condition_name` or `forbid_if: condition_name`

**Future enhancement**: Compound conditions with AND/OR logic (e.g., `require_if: "readme_exists AND has_md_files"`).

**Workaround**: Define compound conditions in `conditions` block:

```yaml
conditions:
  readme_and_files: file_exists("README.md") AND md_files_exist() # Not yet supported
```

Current workaround uses predicate chaining through multiple condition definitions.

## Examples

### Example 1: Simple Template (Minimal Validation)

Basic validation with title and length constraints:

```yaml
---
doc_type: template
template_for: simple-doc
status: active
date_created: 2025-11-08
primary_category: documentation

validation:
  schema_version: 1
  title_pattern: "^# .+ Guide$"
  max_lines: 100
---
# Example Guide

Content goes here...
```

### Example 2: Conditional Sections (CLAUDE.md)

Navigation file with conditional section requirements:

```yaml
---
doc_type: template
template_for: claude-md
status: active
date_created: 2025-11-08
primary_category: documentation

validation:
  schema_version: 1

  conditions:
    readme_exists: file_exists("README.md")
    has_md_files: md_files_exist(exclude=["README.md", "CLAUDE.md"])
    has_subdirs: subdirs_exist()

  title_pattern: "^# .+ Guide for LLM Agents$"
  max_lines: 50

  forbidden:
    - pattern: "(?i)how to"
      reason: "how-to instructions"
      severity: error

  required_sections:
    - name: "Purpose"
      must_exist: true
      max_paragraphs: 3

    - name: "Hub"
      require_if: readme_exists
      content_pattern: '^\*\*`README\.md`\*\* - Read when'

    - name: "Files"
      require_if: has_md_files
      files_rules:
        must_list_all_md: true
        exclude_globs: ["README.md", "CLAUDE.md"]
        entry_pattern: '^\*\*`.+\.md`\*\* - Read when'

    - name: "Subdirectories"
      require_if: has_subdirs
      files_rules:
        must_list_all_subdirs: true
        entry_pattern: '^\*\*`.+/`\*\* - Read when'
---
# Example Directory Guide for LLM Agents

## Purpose

Documentation for example domain.
```

### Example 3: Frontmatter Validation (Hub)

Hub template with comprehensive frontmatter validation:

```yaml
---
doc_type: template
template_for: hub
status: active
date_created: 2025-11-08
primary_category: documentation

validation:
  schema_version: 1

  filename_pattern: "^README\\.md$"

  frontmatter:
    required_fields:
      - doc_type
      - status
      - date_created
      - primary_category
      - consolidated_spokes

    field_constraints:
      doc_type:
        enum: ["hub"]
      status:
        enum: ["draft", "active", "deprecated", "superseded"]
      date_created:
        pattern: "^\\d{4}-\\d{2}-\\d{2}$"
      consolidated_spokes:
        type: array
        min_items: 3

    conditional_constraints:
      - if_field: status
        equals: superseded
        then_required: ["superseded_by"]

  required_sections:
    - name: "Context"
      must_exist: true
      min_paragraphs: 2
      max_paragraphs: 4

    - name: "Decision"
      must_exist: true
      subsections_required:
        min: 3
        max: 7
        pattern: "^### "
---
# Example Architecture

## Context

Explanation of fragmentation problem...
```

### Example 4: File Listings Validation

CLAUDE.md with file listing format enforcement:

```yaml
---
doc_type: template
template_for: claude-md-strict
status: active
date_created: 2025-11-08
primary_category: documentation

validation:
  schema_version: 1

  conditions:
    has_md_files: md_files_exist(exclude=["README.md", "CLAUDE.md"])

  required_sections:
    - name: "Files"
      require_if: has_md_files
      files_rules:
        must_list_all_md: true
        exclude_globs: ["README.md", "CLAUDE.md"]
        entry_pattern: '^\*\*`.+\.md`\*\* - Read when .+$'
---

# Example Guide for LLM Agents

## Files

**`authentication.md`** - Read when implementing auth flows
**`encryption.md`** - Read when implementing encryption
```

### Example 5: Complex Template (All Features)

Comprehensive validation with all rule types:

```yaml
---
doc_type: template
template_for: comprehensive-hub
status: active
date_created: 2025-11-08
primary_category: documentation

validation:
  schema_version: 1

  # Filesystem conditions
  conditions:
    readme_exists: file_exists("README.md")
    has_spokes: md_files_exist(exclude=["README.md"])

  # Filename requirement
  filename_pattern: "^README\\.md$"

  # Frontmatter validation
  frontmatter:
    required_fields:
      - doc_type
      - status
      - consolidated_spokes
    field_constraints:
      doc_type:
        enum: ["hub"]
      consolidated_spokes:
        type: array
        min_items: 3
    conditional_constraints:
      - if_field: status
        equals: superseded
        then_required: ["superseded_by"]

  # Title format
  title_pattern: "^# .+ Architecture$"

  # Length limit
  max_lines: 500

  # Content restrictions
  forbidden:
    - pattern: "(?i)version \\d+"
      reason: "version numbers in content"
      severity: warn
    - pattern: "TODO|FIXME"
      reason: "unresolved tasks"
      severity: error

  # Structure requirements
  required_sections:
    - name: "Context"
      must_exist: true
      min_paragraphs: 2
      max_paragraphs: 4

    - name: "Decision"
      must_exist: true
      subsections_required:
        min: 3
        max: 7
        pattern: "^### "

    - name: "Consequences"
      must_exist: true

    - name: "Related Documents"
      must_exist: true
      files_rules:
        must_list_all_md: true
        exclude_globs: ["README.md"]
        entry_pattern: '^\*\*\[.+\]\(.+\.md\)\*\*'
---
# Security Architecture

## Context

Fragmentation explanation...
```

## Best Practices

### Keep Validation Blocks Concise

Target 20-40 lines for validation blocks. Longer blocks indicate over-specification.

**Good**:

```yaml
validation:
  schema_version: 1
  title_pattern: "^# .+ Guide$"
  max_lines: 100
```

**Avoid**:

```yaml
validation:
  schema_version: 1
  # 100+ lines of hyper-specific rules
```

### Use Conditions for Optional Sections

Prefer conditional requirements over complex forbidden patterns.

**Good**:

```yaml
conditions:
  has_md_files: md_files_exist(exclude=["README.md"])
required_sections:
  - name: "Files"
    require_if: has_md_files
```

**Avoid**:

```yaml
required_sections:
  - name: "Files"
    must_exist: true # Fails for empty directories
```

### Prefer Structural Checks Over Regex

Use section requirements instead of content pattern matching when possible.

**Good**:

```yaml
required_sections:
  - name: "Purpose"
    must_exist: true
    max_paragraphs: 3
```

**Avoid**:

```yaml
forbidden:
  - pattern: "^## Purpose\\n(.*\\n){10,}"
    reason: "Purpose too long"
```

### Write Clear Error Messages

Explain why pattern is forbidden, not just that it is.

**Good**:

```yaml
forbidden:
  - pattern: "(?i)how to"
    reason: "how-to instructions belong in implementation docs, not navigation files"
```

**Avoid**:

```yaml
forbidden:
  - pattern: "(?i)how to"
    reason: "forbidden content"
```

### Use Severity Appropriately

Reserve `error` for standards violations, use `warn` for guidelines.

**Error** (blocks commit):

- Document structure violations
- Missing required sections
- Incorrect frontmatter

**Warn** (informational):

- Style preferences
- Recommended but not required patterns
- Soft guidelines

### Document Complex Patterns

Add comments explaining non-obvious regex patterns.

```yaml
forbidden:
  # Prevent version numbers like "v1.0" or "version 2.3"
  - pattern: "(?i)version\\s+\\d+\\.\\d+"
    reason: "version numbers create maintenance burden"
    severity: warn
```

## Troubleshooting

### Common Validation Errors

**Schema version missing**:

```
Error: validation block missing schema_version field
Fix: Add schema_version: 1 as first field in validation block
```

**Invalid regex pattern**:

```
Error: Invalid regex pattern: unbalanced parenthesis
Fix: Escape special characters: \\( \\) \\. \\[ \\]
```

**Condition name mismatch**:

```
Error: require_if references undefined condition 'readme_exists'
Fix: Add condition to conditions block with matching name
```

**Section name doesn't match**:

```
Error: Required section "Files" not found
Fix: Ensure section heading exactly matches name field (case-sensitive)
```

### Debugging Validation Rules

**Use --show-effective-rules flag**:

```bash
./validate.py template --show-effective-rules doc/_meta/02-templates/hub.md
```

Displays resolved validation rules after template inheritance.

**Check validation_schema.json**:

Validate template frontmatter against JSON schema:

```bash
# Extract validation block and validate
python3 -c "import json, yaml; print(json.dumps(yaml.safe_load(open('template.md').read().split('---')[1])['validation']))" | jsonschema -i /dev/stdin validation_schema.json
```

**Test with minimal fixtures**:

Create minimal test documents to isolate failures:

```yaml
---
doc_type: test
status: active
date_created: 2025-11-08
primary_category: testing
---
# Test Guide

## Purpose

Minimal test document.
```

**Validate template frontmatter independently**:

Extract and validate just the validation block:

```python
import yaml
with open('template.md') as f:
    frontmatter = yaml.safe_load(f.read().split('---')[1])
    validation = frontmatter.get('validation', {})
    assert 'schema_version' in validation
    assert validation['schema_version'] == 1
```

### Schema Validation Failures

**Run through JSON Schema validator**:

```bash
python3 -c "
import json, yaml, sys
from pathlib import Path
content = Path('template.md').read_text()
fm = yaml.safe_load(content.split('---')[1])
print(json.dumps(fm['validation'], indent=2))
"
```

**Check for typos in field names**:

Common typos:

- `required_section` → `required_sections`
- `condition` → `conditions`
- `frontmater` → `frontmatter`
- `pattern_title` → `title_pattern`

**Verify enum values are arrays**:

```yaml
# Wrong
field_constraints:
  doc_type:
    enum: "hub"  # String instead of array

# Correct
field_constraints:
  doc_type:
    enum: ["hub"]
```

**Ensure required fields present**:

All validation blocks require `schema_version`:

```yaml
validation:
  schema_version: 1 # Required
  title_pattern: "^# .+ Guide$"
```

## Related Documents

**Hub Document**: This document is part of the Tooling architecture. See [README.md](README.md) for validation system overview and relationships to linters and architecture documentation.

**Dependencies**:

- **validation_schema.json**: JSON Schema defining validation block structure
- **rule_evaluator.py**: Python module executing validation rules
- **predicates.py**: Predicate evaluation engine for conditional logic
- **markdown_parser.py**: AST parsing for section validation

**References**:

- **doc/\_meta/02-templates/README.md**: How templates use validation blocks
- **doc/\_meta/01-standards/frontmatter-reference.md**: Frontmatter field definitions
- **doc/\_meta/04-tooling/architecture.md**: Validation system architecture
