# Developer Handoff: Template-Driven CLAUDE.md Validation

**Status**: Documentation complete, ready for code implementation
**Developer task**: Integrate template-driven validation into validate.py

## What Was Completed (Documentation Agent)

1. ✅ Added validation frontmatter to `/doc/_meta/02-templates/claude-md.md`
2. ✅ Created 10 test fixtures (4 valid, 6 invalid) in this directory
3. ✅ Validated frontmatter against JSON Schema
4. ✅ Documented all test cases and expected behaviors

## What Needs Implementation (Developer Agent)

### 1. Implement Predicate Functions

The validation frontmatter uses three conditional predicates that need implementation:

```python
# In validate.py or a new predicates.py module

def file_exists(doc_path: Path, filename: str) -> bool:
    """Check if file exists in same directory as document.

    Args:
        doc_path: Path to the document being validated
        filename: Name of file to check (e.g., "README.md")

    Returns:
        True if file exists, False otherwise
    """
    return (doc_path.parent / filename).exists()


def md_files_exist(doc_path: Path, exclude: list[str] = None) -> bool:
    """Check if .md files exist in directory (excluding patterns).

    Args:
        doc_path: Path to the document being validated
        exclude: List of filenames to exclude (default: [])

    Returns:
        True if at least one .md file exists (after exclusions)
    """
    exclude = exclude or []
    directory = doc_path.parent
    md_files = [f for f in directory.glob('*.md')
                if f.name not in exclude]
    return len(md_files) > 0


def subdirs_exist(doc_path: Path) -> bool:
    """Check if subdirectories exist in directory.

    Args:
        doc_path: Path to the document being validated

    Returns:
        True if at least one subdirectory exists (excluding hidden/dunder)
    """
    directory = doc_path.parent
    subdirs = [d for d in directory.iterdir()
               if d.is_dir()
               and not d.name.startswith('.')
               and not d.name.startswith('__')]
    return len(subdirs) > 0
```

### 2. Update RuleEvaluator to Support New Features

The validation frontmatter uses several features that may need implementation:

#### A. Conditional Sections (require_if)

```python
# Current: required_sections with must_exist
# New: required_sections with require_if (conditional)

{
  "name": "Hub",
  "require_if": "readme_exists",
  "content_pattern": "..."
}

# Implementation approach:
# 1. Parse conditions block from template
# 2. Evaluate condition predicate (readme_exists)
# 3. If true, apply section requirement
# 4. If false, skip section validation
```

#### B. Files Rules

```python
# Section validation with file listing requirements

{
  "name": "Files",
  "require_if": "has_md_files",
  "files_rules": {
    "must_list_all_md": true,
    "exclude_globs": ["README.md", "CLAUDE.md"],
    "entry_pattern": "^\\*\\*`.+\\.md`\\*\\* - Read when"
  }
}

# Implementation approach:
# 1. Find section in document
# 2. Get list of .md files in directory (excluding patterns)
# 3. Verify each file is listed in section
# 4. Verify each listing matches entry_pattern
```

#### C. Content Pattern Validation

```python
# Section must contain content matching pattern

{
  "name": "Hub",
  "content_pattern": "^\\*\\*`README\\.md`\\*\\* - Read when"
}

# Implementation: Search section content for pattern
```

### 3. Refactor validate_claude_md() Function

Current implementation (lines 470-602) should be replaced with:

```python
def validate_claude_md(args) -> int:
    """Validate CLAUDE.md files using template-driven rules."""
    # 1. Load template from doc/_meta/02-templates/claude-md.md
    template_path = Path('doc/_meta/02-templates/claude-md.md')
    template = load_template(template_path)

    # 2. Extract validation block from template frontmatter
    validation_rules = template['frontmatter']['validation']

    # 3. Discover all CLAUDE.md files
    claude_files = discover_claude_md()

    # 4. Validate each file using RuleEvaluator
    errors = []
    for claude_path in claude_files:
        content = claude_path.read_text(encoding='utf-8')

        # Use RuleEvaluator with template rules
        evaluator = RuleEvaluator(validation_rules)
        validation_errors = evaluator.evaluate(claude_path, content)

        for ve in validation_errors:
            errors.append(ve.format_error())

    # 5. Report results
    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1

    print(f"CLAUDE.md validation passed: {len(claude_files)} files")
    return 0
```

### 4. Testing Requirements

Test against all fixtures in this directory:

```bash
# Expected results:
./validate.py --claude-md doc/scripts/fixtures/claude-md/valid-minimal.md
# PASS

./validate.py --claude-md doc/scripts/fixtures/claude-md/valid-with-hub.md
# PASS

./validate.py --claude-md doc/scripts/fixtures/claude-md/valid-with-files.md
# PASS

./validate.py --claude-md doc/scripts/fixtures/claude-md/valid-complete.md
# PASS

./validate.py --claude-md doc/scripts/fixtures/claude-md/invalid-title.md
# FAIL: Title doesn't match pattern

./validate.py --claude-md doc/scripts/fixtures/claude-md/invalid-forbidden-pattern.md
# FAIL: 3 forbidden patterns detected

./validate.py --claude-md doc/scripts/fixtures/claude-md/invalid-missing-purpose.md
# FAIL: Missing required Purpose section

./validate.py --claude-md doc/scripts/fixtures/claude-md/invalid-too-long.md
# FAIL: Exceeds max_lines (50)

./validate.py --claude-md doc/scripts/fixtures/claude-md/invalid-purpose-too-verbose.md
# FAIL: Purpose exceeds max_paragraphs (3)

./validate.py --claude-md doc/scripts/fixtures/claude-md/invalid-missing-trigger.md
# FAIL: File entries missing " - Read when " trigger
```

### 5. Validation Against Existing Files

After fixtures pass, validate all existing CLAUDE.md files:

```bash
./validate.py --claude-md
# Should validate all CLAUDE.md files in repo
# All should pass (if not, either fix files or adjust rules)
```

## Implementation Priority

1. **High**: Predicate functions (required for conditional logic)
2. **High**: Conditional section validation (require_if)
3. **High**: File listing validation (must_list_all_md, must_list_all_subdirs)
4. **Medium**: Entry pattern validation
5. **Medium**: Content pattern validation
6. **Low**: max_paragraphs validation (already exists?)

## Expected Errors After Implementation

Based on fixtures, the error messages should be:

1. **invalid-title.md**: `Title doesn't match required pattern: ^# .+ Guide for LLM Agents$`
2. **invalid-forbidden-pattern.md**:
   - `Forbidden pattern detected: "contains information" (explanatory content)`
   - `Forbidden pattern detected: "how to" (how-to instructions)`
   - `Forbidden pattern detected: "step 1" (step-by-step procedures)`
3. **invalid-missing-purpose.md**: `Missing required section: Purpose`
4. **invalid-too-long.md**: `Document exceeds max_lines limit (55 > 50)`
5. **invalid-purpose-too-verbose.md**: `Section "Purpose" exceeds max_paragraphs (4 > 3)`
6. **invalid-missing-trigger.md**:
   - `File "example.md" entry doesn't match pattern: ^\*\*`.+\.md`\*\* - Read when`
   - `File "another.md" entry doesn't match pattern: ^\*\*`.+\.md`\*\* - Read when`

## Questions for Developer

1. Should predicate functions go in `validate.py` or separate `predicates.py`?
2. Should RuleEvaluator be extended or create new TemplateValidator class?
3. How to handle error message formatting for better UX?
4. Should validation rules be cached (template parsed once)?

## Success Criteria

✅ All 4 valid fixtures pass validation
✅ All 6 invalid fixtures fail with correct error messages
✅ All existing CLAUDE.md files in repo pass validation
✅ Hardcoded validation removed from validate.py (lines 470-602)
✅ Code is maintainable and extensible for M7-M10

## References

- Template: `/doc/_meta/02-templates/claude-md.md`
- JSON Schema: `/doc/scripts/validation_schema.json`
- Existing validation: `/doc/scripts/validate.py` (lines 470-602)
- Test fixtures: This directory
- Completion report: `MILESTONE-6-COMPLETION.md`
