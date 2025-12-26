# CLAUDE.md Validation Test Fixtures

Test fixtures for validating CLAUDE.md files against the validation rules defined in `/doc/_meta/02-templates/claude-md.md`.

## Valid Fixtures

### `valid-minimal.md`
Minimal valid CLAUDE.md with only the required Purpose section. Tests:
- Valid title pattern
- Required Purpose section exists
- No forbidden patterns
- Within line limit

### `valid-with-hub.md`
CLAUDE.md with Hub section. Tests:
- Hub section with proper README.md entry
- Proper trigger pattern: " - Read when "

### `valid-with-files.md`
CLAUDE.md with Files section. Tests:
- Multiple file entries with proper triggers
- Entry pattern: `**\`filename.md\`** - Read when`

### `valid-complete.md`
Complete CLAUDE.md with all sections. Tests:
- Purpose, Hub, Files, and Subdirectories sections
- All sections follow proper patterns
- Subdirectory entries with trailing slash: `dirname/`

## Invalid Fixtures

### `invalid-title.md`
**Expected Error**: Title doesn't match pattern `^# .+ Guide for LLM Agents$`

Tests that title validation catches improper titles.

### `invalid-forbidden-pattern.md`
**Expected Errors**: Multiple forbidden patterns detected:
- "contains information" (explanatory content)
- "how to" (how-to instructions)
- "step 1" (step-by-step procedures)

Tests that forbidden pattern detection works correctly.

### `invalid-missing-purpose.md`
**Expected Error**: Missing required Purpose section

Tests that required section validation works (has ## Overview instead).

### `invalid-too-long.md`
**Expected Error**: Exceeds max_lines limit (50 lines)

Tests line limit validation with 55+ lines.

### `invalid-purpose-too-verbose.md`
**Expected Error**: Purpose section exceeds max_paragraphs (3)

Tests paragraph counting within a section.

### `invalid-missing-trigger.md`
**Expected Error**: File entries missing " - Read when " trigger pattern

Tests that entry_pattern validation catches improper triggers.

## Test Coverage

These fixtures cover:
- ✅ Title pattern validation
- ✅ Required sections (Purpose)
- ✅ Conditional sections (Hub, Files, Subdirectories)
- ✅ Forbidden patterns (4 patterns)
- ✅ Line limits (max_lines)
- ✅ Section constraints (max_paragraphs)
- ✅ Entry patterns (content_pattern, entry_pattern)

## Test Directory Structure

Valid fixtures require isolated test directories with companion files to properly test file and subdirectory listing validation:

- `test-valid-minimal/` - Contains only `CLAUDE.md` (no companion files needed)
- `test-valid-with-hub/` - Contains `CLAUDE.md` and `README.md`
- `test-valid-with-files/` - Contains `CLAUDE.md`, `README.md`, `architecture.md`, `configuration.md`, `deployment.md`
- `test-valid-complete/` - Contains `CLAUDE.md`, `README.md`, `getting-started.md`, `api-reference.md`, and subdirectories `core/`, `integrations/`, `testing/`

Invalid fixtures are tested directly from this directory since they should fail validation regardless of surrounding files.

## Running Tests

### Manual Testing

Test all fixtures:
```bash
cd doc/scripts/fixtures/claude-md
python3 ../../validate.py validate-claude-md
```

### Automated Test Script

```python
from pathlib import Path
from validate import load_template_validation
from rule_evaluator import RuleEvaluator

template_path = Path('doc/_meta/02-templates/claude-md.md')
validation_rules = load_template_validation(template_path)

# Test valid fixtures in isolated directories
for test_dir in ['test-valid-minimal', 'test-valid-with-hub',
                  'test-valid-with-files', 'test-valid-complete']:
    claude_path = Path(f'doc/scripts/fixtures/claude-md/{test_dir}/CLAUDE.md')
    content = claude_path.read_text()
    evaluator = RuleEvaluator(validation_rules)
    errors = evaluator.evaluate(claude_path, content)
    print(f"{'PASS' if not errors else 'FAIL'}: {test_dir}")

# Test invalid fixtures directly
for fixture in ['invalid-title.md', 'invalid-forbidden-pattern.md',
                'invalid-missing-purpose.md', 'invalid-too-long.md',
                'invalid-purpose-too-verbose.md', 'invalid-missing-trigger.md']:
    claude_path = Path(f'doc/scripts/fixtures/claude-md/{fixture}')
    content = claude_path.read_text()
    evaluator = RuleEvaluator(validation_rules)
    errors = evaluator.evaluate(claude_path, content)
    print(f"{'PASS' if errors else 'FAIL'}: {fixture}")
```

## Usage

These fixtures are used by the validation test suite to ensure that:
1. Template validation frontmatter passes JSON Schema validation
2. validate.py correctly parses validation rules from template
3. Valid fixtures pass validation
4. Invalid fixtures fail with appropriate error messages
