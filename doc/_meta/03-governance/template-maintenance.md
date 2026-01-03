---
doc_type: spoke
status: active
date_created: 2025-11-08
primary_category: governance
hub_document: doc/_meta/03-governance/README.md
tags:
  - templates
  - validation
  - maintenance
  - procedures
maintainer: Documentation Team
---

# Template Maintenance Guide

## Overview

This guide provides procedures for creating new documentation templates with validation rules and updating existing template validation. Templates define structure and validation rules for documentation types (hub, spoke, CLAUDE.md, cross-cutting indexes). Validation rules use a declarative DSL embedded in template frontmatter.

**Audience**: Template maintainers, documentation team members creating new document types.

**Prerequisites**: Familiarity with YAML frontmatter, basic regex patterns, documentation standards.

## Creating New Templates with Validation

### Step-by-Step Process

**1. Identify Template Need**

Determine when a new template is needed:

- New document type emerges (e.g., API reference, tutorial)
- Existing document type requires stricter validation
- Specialized variation of existing template needed

**2. Copy Base Template**

Start with the closest existing template:

```bash
cd doc/_meta/02-templates
cp hub.md my-new-template.md
```

**3. Define Template Frontmatter**

Update frontmatter with template-specific fields:

```yaml
---
doc_type: template
template_for: my-new-doc-type
status: active
date_created: 2025-11-08
primary_category: documentation
maintainer: Documentation Team

validation:
  schema_version: 1
  # Validation rules go here
---
```

**Required frontmatter fields**:

- `doc_type: template` - Identifies file as template
- `template_for: <type>` - Document type this template creates
- `status: active` - Template lifecycle state
- `date_created: YYYY-MM-DD` - Creation date
- `primary_category: documentation` - Category classification
- `validation.schema_version: 1` - DSL version

**4. Define Validation Rules**

Add validation rules incrementally, testing each addition:

```yaml
validation:
  schema_version: 1

  # Start with basic rules
  title_pattern: "^# .+ Guide$"
  max_lines: 200

  # Add frontmatter validation
  frontmatter:
    required_fields:
      - doc_type
      - status
      - date_created
    field_constraints:
      status:
        enum: ["draft", "active", "deprecated"]

  # Add section requirements
  required_sections:
    - name: "Overview"
      must_exist: true
      max_paragraphs: 3
```

**5. Test Validation Rules Locally**

Create a test fixture document:

```bash
cd doc/scripts/fixtures
mkdir -p test_new_template
cat > test_new_template/example.md << 'EOF'
---
doc_type: my-new-doc-type
status: active
date_created: 2025-11-08
primary_category: testing
---

# Example Guide

## Overview

Test content here.
EOF
```

Run validation against fixture:

```bash
cd doc/scripts
python3 validate.py validate-all
```

**6. Iterate Until Validation Passes**

Fix validation errors:

- Adjust patterns that are too strict
- Loosen constraints that fail legitimate documents
- Add conditional logic for optional sections

Use `--show-effective-rules` to debug:

```bash
python3 validate.py validate-hub --show-effective-rules doc/_meta/02-templates/my-new-template.md
```

**7. Document Template Usage**

Add inline comments explaining validation rules:

```yaml
validation:
  schema_version: 1

  # Enforce consistent title format
  title_pattern: "^# .+ Guide$"

  # Limit document length to encourage focus
  max_lines: 200
```

Add template usage instructions in template body:

```markdown
# [Document Title] Guide

<!-- Replace [Document Title] with specific name -->

## Overview

<!-- Provide 2-3 paragraphs explaining purpose -->
```

**8. Update Template Hub**

Add new template to `doc/_meta/02-templates/README.md`:

```markdown
### My New Document Type Template

Template for creating [description] documents.

**Key Points:**

- [Primary usage scenario]
- [Validation requirements]
- [Special considerations]

**Cross-References:**

- **my-new-template.md**: Complete template with validation rules
```

**9. Commit Template**

Create focused commit:

```bash
git add doc/_meta/02-templates/my-new-template.md
git add doc/_meta/02-templates/README.md
git add doc/scripts/fixtures/test_new_template/
git commit -m "Add template for my-new-doc-type documents

Introduces validation for [description] including:
- Title pattern enforcement
- Frontmatter schema validation
- Required sections structure

Includes test fixture demonstrating template usage."
```

### Validation Frontmatter Structure

Validation blocks follow this structure:

```yaml
validation:
  schema_version: 1

  # Optional: Conditional logic predicates
  conditions:
    readme_exists: file_exists("README.md")
    has_files: md_files_exist(exclude=["README.md"])

  # Optional: Filename pattern
  filename_pattern: "^README\\.md$"

  # Optional: Title validation
  title_pattern: "^# .+ Architecture$"

  # Optional: Length limit
  max_lines: 500

  # Optional: Frontmatter validation
  frontmatter:
    required_fields: [...]
    field_constraints: { ... }
    conditional_constraints: [...]

  # Optional: Content restrictions
  forbidden:
    - pattern: "..."
      reason: "..."
      severity: error

  # Optional: Section structure
  required_sections:
    - name: "Section Name"
      must_exist: true
      min_paragraphs: 2
```

See [validation-dsl-reference.md](../04-tooling/validation-dsl-reference.md) for complete DSL documentation.

### Example Workflow

**Scenario**: Create template for API reference documents.

```bash
# 1. Copy base template
cd doc/_meta/02-templates
cp spoke.md api-reference.md

# 2. Edit frontmatter
cat > api-reference.md << 'EOF'
---
doc_type: template
template_for: api-reference
status: active
date_created: 2025-11-08
primary_category: documentation
maintainer: Documentation Team

validation:
  schema_version: 1
  title_pattern: "^# .+ API Reference$"
  max_lines: 300

  frontmatter:
    required_fields:
      - doc_type
      - status
      - api_version
    field_constraints:
      doc_type:
        enum: ["api-reference"]
      api_version:
        pattern: "^v\\d+\\.\\d+$"

  required_sections:
    - name: "Endpoints"
      must_exist: true
    - name: "Authentication"
      must_exist: true
    - name: "Error Codes"
      must_exist: true
---

# [Service Name] API Reference
...
EOF

# 3. Create test fixture
mkdir -p ../scripts/fixtures/test_api_reference
cat > ../scripts/fixtures/test_api_reference/sensor-api.md << 'EOF'
---
doc_type: api-reference
status: active
date_created: 2025-11-08
primary_category: api
api_version: v1.0
hub_document: README.md
---

# Sensor API Reference

## Endpoints
...
## Authentication
...
## Error Codes
...
EOF

# 4. Test validation
cd ../../scripts
python3 validate.py validate-all

# 5. Fix any errors and iterate
# 6. Commit when validation passes
```

## Updating Existing Template Validation

### When to Update Validation Rules

Update template validation when:

- Documentation standards evolve
- New anti-patterns discovered
- Validation too strict (causes false positives)
- Validation too loose (allows non-compliant documents)
- Conditional logic needed for new use cases

**Don't update when**:

- Single document violates template (fix document instead)
- Personal preference (standards trump preference)
- Edge case affects <5% of documents (handle manually)

### How to Modify Validation Frontmatter

**1. Document Current State**

Run validation to establish baseline:

```bash
cd doc/scripts
python3 validate.py validate-all > baseline.txt 2>&1
```

**2. Edit Template Validation**

Make focused changes to validation block:

```yaml
# Before
validation:
  schema_version: 1
  max_lines: 200

# After
validation:
  schema_version: 1
  max_lines: 300  # Increased limit for complex hubs
```

**3. Test Changes**

Re-run validation:

```bash
python3 validate.py validate-all
```

Compare against baseline:

```bash
python3 validate.py validate-all > updated.txt 2>&1
diff baseline.txt updated.txt
```

**4. Verify Impact**

Check which documents are affected:

```bash
# Find all documents using this template
grep -r "doc_type: hub" doc/ --include="*.md" | wc -l
```

Manually review sample documents to ensure they still validate correctly.

**5. Update Documentation**

Update inline comments explaining the change:

```yaml
validation:
  schema_version: 1
  max_lines: 300 # Increased from 200 to accommodate security hub complexity
```

**6. Commit Changes**

Create descriptive commit:

```bash
git add doc/_meta/02-templates/hub.md
git commit -m "Increase hub max_lines limit from 200 to 300

Security hub exceeded 200 lines due to comprehensive threat
model documentation. Increasing limit to 300 maintains conciseness
while accommodating detailed security content.

Affects 12 hub documents, all pass updated validation."
```

### Testing Changes

**Run Full Validation Suite**:

```bash
cd doc/scripts
python3 validate.py validate-all
```

**Test Specific Template**:

```bash
python3 validate.py validate-hub
python3 validate.py validate-spoke
python3 validate.py validate-claude-md
```

**Use Debug Flags**:

```bash
# Show effective validation rules after processing
python3 validate.py validate-hub --show-effective-rules

# Verbose output for debugging
python3 validate.py validate-all --verbose
```

**Create Test Fixtures**:

Add edge case fixtures to `doc/scripts/fixtures/`:

```bash
mkdir -p doc/scripts/fixtures/edge_case_hub
cat > doc/scripts/fixtures/edge_case_hub/README.md << 'EOF'
---
doc_type: hub
status: active
date_created: 2025-11-08
primary_category: testing
consolidated_spokes:
  - spoke1.md
  - spoke2.md
  - spoke3.md
---

# Edge Case Architecture

...
EOF

python3 validate.py validate-hub doc/scripts/fixtures/edge_case_hub/README.md
```

### Backwards Compatibility Considerations

**Breaking Changes**:

- Adding new required frontmatter fields
- Making optional sections mandatory
- Tightening pattern constraints
- Reducing max_lines limit

**Non-Breaking Changes**:

- Adding optional validation rules
- Loosening constraints
- Increasing max_lines limit
- Adding conditional logic with `require_if`

**Handle Breaking Changes**:

1. **Audit Impact**: Find all affected documents

   ```bash
   grep -r "doc_type: hub" doc/ --include="*.md"
   ```

2. **Fix Documents First**: Update documents to comply with new rules

3. **Update Template**: Apply stricter validation after documents fixed

4. **Commit Together**: Group document updates and template changes in same PR

**Example - Adding Required Field**:

```bash
# 1. Add field to all hub documents
for hub in $(find doc -name "README.md"); do
  # Add maintainer field to frontmatter
  sed -i '' '/^tags:/a\
maintainer: Documentation Team' "$hub"
done

# 2. Update template validation
# Add maintainer to required_fields in doc/_meta/02-templates/hub.md

# 3. Test all documents pass
python3 validate.py validate-all

# 4. Commit together
git add doc/**/README.md doc/_meta/02-templates/hub.md
git commit -m "Add required maintainer field to hub documents"
```

## Testing Validation Rules Locally

### Running validate.py Commands

**Validate All Documentation**:

```bash
cd doc/scripts
python3 validate.py validate-all
```

**Validate Specific Document Types**:

```bash
# Hub documents only
python3 validate.py validate-hub

# Spoke documents only
python3 validate.py validate-spoke

# CLAUDE.md navigation files only
python3 validate.py validate-claude-md

# Cross-cutting indexes only
python3 validate.py validate-cross-cutting-index
```

**Validate Individual Document**:

```bash
python3 validate.py validate-hub doc/06-security/README.md
```

**Check Validation Complexity**:

```bash
python3 validate.py check-complexity
```

### Creating Test Fixtures

Test fixtures demonstrate template usage and validate edge cases.

**Fixture Structure**:

```
doc/scripts/fixtures/
  test_hub_basic/
    README.md          # Minimal valid hub
    spoke1.md
    spoke2.md
    spoke3.md
  test_hub_complex/
    README.md          # Hub with all optional features
    spoke1.md
    ...
  test_claude_md/
    CLAUDE.md          # Navigation file with subdirs
    file1.md
    subdir/
```

**Create Test Fixture**:

```bash
cd doc/scripts/fixtures
mkdir -p test_my_template
cat > test_my_template/example.md << 'EOF'
---
doc_type: my-doc-type
status: active
date_created: 2025-11-08
primary_category: testing
---

# Example Document

## Required Section

Content here.
EOF
```

**Run Validation on Fixture**:

```bash
cd doc/scripts
python3 validate.py validate-all
```

### Using --show-effective-rules Flag

Display resolved validation rules for debugging:

```bash
cd doc/scripts
python3 validate.py validate-hub --show-effective-rules doc/_meta/02-templates/hub.md
```

**Output Format**:

```
Template: doc/_meta/02-templates/hub.md
Effective Validation Rules:
{
  "schema_version": 1,
  "filename_pattern": "^README\\.md$",
  "title_pattern": "^# .+ Architecture$",
  "max_lines": 500,
  "frontmatter": {
    "required_fields": ["doc_type", "status", ...],
    ...
  },
  ...
}
```

**Use Cases**:

- Verify conditional logic resolves correctly
- Debug complex pattern matching
- Understand inheritance (if implemented)
- Confirm DSL parsing

### Debugging Validation Errors

**Read Error Messages Carefully**:

```
[ERROR] doc/06-security/README.md:15: missing_required_section
  Detail: Required section "Decision" not found
  Expected: Section with heading "## Decision"
  Found: Sections present: Context, Consequences, Related Documents
```

**Error Components**:

- File path and line number
- Rule violation identifier
- Detailed explanation
- Expected vs actual values

**Common Debugging Steps**:

1. **Check exact section name**: Section headings are case-sensitive

   ```markdown
   # Wrong

   ## decision

   # Correct

   ## Decision
   ```

2. **Verify frontmatter syntax**: YAML indentation matters

   ```yaml
   # Wrong
   tags:
   - security

   # Correct
   tags:
     - security
   ```

3. **Test regex patterns**: Use online regex testers for complex patterns

4. **Check file encoding**: Ensure UTF-8 encoding

   ```bash
   file -I document.md
   ```

5. **Validate YAML**: Use yamllint for frontmatter debugging
   ```bash
   yamllint doc/_meta/02-templates/hub.md
   ```

## Complexity Management

### When Validation Blocks Become Too Complex

**Warning Signs**:

- Validation block exceeds 50 lines
- More than 5 conditional predicates
- More than 10 required sections
- More than 8 forbidden patterns
- Nested conditionals with complex boolean logic

**Check Complexity**:

```bash
cd doc/scripts
python3 validate.py check-complexity
```

**Output**:

```
Template Complexity Report:

doc/_meta/02-templates/hub.md:
  Frontmatter lines: 38 (limit: 40) [PASS]
  Conditions: 3 (limit: 5) [PASS]
  Required sections: 7 (limit: 10) [PASS]
  Forbidden patterns: 4 (limit: 8) [PASS]

doc/_meta/02-templates/claude-md.md:
  Frontmatter lines: 52 (limit: 40) [WARN]
  Conditions: 6 (limit: 5) [WARN]
  Required sections: 4 (limit: 10) [PASS]
  Forbidden patterns: 3 (limit: 8) [PASS]
```

### Extracting Complex Rules to Python Validators (Future)

**When DSL becomes insufficient**:

- Need custom validation logic
- Complex cross-document validation
- Performance-critical validation
- Multi-file consistency checks

**Future Enhancement**:

Create custom Python validator in `doc/scripts/custom_validators/`:

```python
# doc/scripts/custom_validators/security_hub_validator.py
from typing import List
from pathlib import Path

def validate_security_hub(doc_path: Path) -> List[str]:
    """Custom validation for security hub documents."""
    errors = []

    # Complex validation logic here
    content = doc_path.read_text()

    # Example: Verify all referenced threat models exist
    threat_refs = extract_threat_model_refs(content)
    for ref in threat_refs:
        if not verify_threat_model_exists(ref):
            errors.append(f"Referenced threat model {ref} not found")

    return errors
```

Register in `validate.py`:

```python
from custom_validators.security_hub_validator import validate_security_hub

# In validation logic
if doc_type == 'hub' and primary_category == 'security':
    custom_errors = validate_security_hub(doc_path)
    all_errors.extend(custom_errors)
```

**Note**: This is a future enhancement. Current implementation uses DSL only.

### Using Shared Rulesets with `extends` (Future)

**Problem**: Common validation rules duplicated across templates.

**Future Solution**: Template inheritance via `extends` field.

```yaml
# Base template: common-spoke.md
validation:
  schema_version: 1
  base_rules:
    title_pattern: "^# "
    max_lines: 200

# Specialized template: api-reference.md
validation:
  schema_version: 1
  extends: common-spoke.md
  title_pattern: "^# .+ API Reference$"  # Overrides base
```

**Benefits**:

- Reduce duplication
- Centralize common rules
- Easier bulk updates

**Implementation Status**: Not yet implemented. Track in future enhancement backlog.

### Complexity Limits and Warnings

**Hard Limits** (validation fails):

- `schema_version` must be 1
- Regex patterns must compile
- Predicate functions must exist
- Field references must be valid

**Soft Limits** (warnings only):

- `max_frontmatter_lines: 40` - Validation block size
- `max_conditions: 5` - Conditional predicates
- `max_required_sections: 10` - Section requirements
- `max_forbidden_patterns: 8` - Forbidden content patterns

**Configure Limits** (in `validate.py`):

```python
COMPLEXITY_LIMITS = {
    'max_frontmatter_lines': 40,
    'max_conditions': 5,
    'max_required_sections': 10,
    'max_forbidden_patterns': 8,
}
```

**Override for Specific Templates** (future):

```yaml
validation:
  schema_version: 1
  complexity_override: true # Suppress warnings
  max_frontmatter_lines: 60 # Justify in comment why needed
```

## Common Patterns and Anti-Patterns

### Good: Structural Checks

**Pattern**: Validate document structure, not content.

```yaml
# Good - Checks section exists and paragraph count
required_sections:
  - name: "Purpose"
    must_exist: true
    max_paragraphs: 3
```

**Why Good**:

- Fast validation (no complex parsing)
- Clear error messages
- Easy to maintain
- Minimal false positives

### Bad: Over-Reliance on Regex Patterns

**Anti-Pattern**: Using regex to validate content semantics.

```yaml
# Bad - Fragile regex checking for specific wording
forbidden:
  - pattern: "^## Purpose\\n\\nThis document (describes|explains|documents)"
    reason: "must use specific opening phrase"
    severity: error
```

**Why Bad**:

- Brittle (breaks with minor wording changes)
- Difficult to debug
- Creates false positives
- Discourages natural writing

**Better Alternative**:

```yaml
# Better - Check structure, not exact wording
required_sections:
  - name: "Purpose"
    must_exist: true
    min_paragraphs: 1
    max_paragraphs: 3
```

### Good: Conditional Logic for Optional Sections

**Pattern**: Sections required only when applicable.

```yaml
# Good - Files section only required if files exist
conditions:
  has_md_files: md_files_exist(exclude=["README.md", "CLAUDE.md"])

required_sections:
  - name: "Files"
    require_if: has_md_files
    files_rules:
      must_list_all_md: true
```

**Why Good**:

- Adapts to directory structure
- No false positives for empty directories
- Encourages complete documentation when files exist

### Bad: Complex Nested Conditions

**Anti-Pattern**: Deeply nested conditional logic.

```yaml
# Bad - Complex boolean logic (not yet supported)
conditions:
  complex_condition: "(file_exists('README.md') AND md_files_exist()) OR (subdirs_exist() AND NOT file_exists('.no-docs'))"

required_sections:
  - name: "Navigation"
    require_if: complex_condition
```

**Why Bad**:

- Hard to understand
- Difficult to debug
- Error-prone
- May not be supported by DSL

**Better Alternative**:

```yaml
# Better - Simple, explicit conditions
conditions:
  readme_exists: file_exists("README.md")
  has_files: md_files_exist()
  has_subdirs: subdirs_exist()

required_sections:
  - name: "Hub"
    require_if: readme_exists

  - name: "Files"
    require_if: has_files

  - name: "Subdirectories"
    require_if: has_subdirs
```

### Good: Clear Error Messages

**Pattern**: Explain why pattern is forbidden.

```yaml
# Good - Explains rationale
forbidden:
  - pattern: "(?i)how to"
    reason: "how-to instructions belong in implementation docs, not navigation files"
    severity: error
```

**Why Good**:

- Educates users
- Justifies restriction
- Helps users fix violations
- Documents design decisions

### Bad: Cryptic Patterns Without Reasons

**Anti-Pattern**: Regex without explanation.

```yaml
# Bad - No explanation, cryptic pattern
forbidden:
  - pattern: "\\b(step|procedure|workflow)\\s+\\d+"
    reason: "forbidden content"
    severity: error
```

**Why Bad**:

- Users don't understand why violation matters
- Difficult to maintain (what was intent?)
- Hard to decide if pattern should change

**Better Alternative**:

```yaml
# Better - Clear explanation
forbidden:
  - pattern: "(?i)step\\s+\\d+"
    reason: "numbered procedures belong in tutorials, not architectural docs; use bullet points for lists"
    severity: error
```

### Pattern Summary

**Do**:

- Check document structure (sections, paragraphs)
- Use conditional logic for optional content
- Write clear, explanatory error messages
- Keep validation blocks focused (<40 lines)
- Test validation rules with fixtures

**Don't**:

- Validate specific wording or phrasing
- Create complex nested conditions
- Use cryptic regex patterns
- Over-specify validation rules
- Forget to document why rules exist

## Related Documents

**Hub Document**: This spoke is part of the Governance hub. See [README.md](README.md) for governance strategy overview and related procedures.

**Dependencies**:

- **doc/\_meta/04-tooling/validation-dsl-reference.md**: Complete DSL syntax and predicate reference
- **doc/\_meta/02-templates/README.md**: Template hub listing all available templates
- **doc/\_meta/01-standards/documentation-standards.md**: Documentation quality standards

**References**:

- **doc/\_meta/04-tooling/architecture.md**: Validation system architecture
- **doc/\_meta/03-governance/documentation-evolution-principle.md**: When to create new templates
