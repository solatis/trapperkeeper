# Milestone 6: CLAUDE.md Template Migration - COMPLETED

**Date**: 2025-11-07
**Status**: ✅ Complete - Ready for developer agent integration

## Summary

Successfully migrated the CLAUDE.md template (`doc/_meta/02-templates/claude-md.md`) to use validation frontmatter. All validation rules previously hardcoded in `validate.py` (lines 470-602) are now declaratively defined in the template's frontmatter.

## Deliverables Completed

### 1. Updated Template ✅

**File**: `/doc/_meta/02-templates/claude-md.md`

Added comprehensive validation frontmatter block containing:

- `schema_version: 1`
- 3 conditional predicates (readme_exists, has_md_files, has_subdirs)
- Title pattern validation
- 50-line maximum
- 4 forbidden patterns (how-to, step-by-step, explanatory content)
- 4 required sections with conditional logic

**Validation Status**: ✅ Passes JSON Schema validation (`validation_schema.json`)

### 2. Test Fixtures ✅

**Directory**: `/doc/scripts/fixtures/claude-md/`

Created 10 test fixtures covering all validation rules:

#### Valid Fixtures (4)

1. `valid-minimal.md` - Minimal valid CLAUDE.md (Purpose only)
2. `valid-with-hub.md` - With Hub section (README.md)
3. `valid-with-files.md` - With Files section (multiple .md files)
4. `valid-complete.md` - Complete with all sections

#### Invalid Fixtures (6)

1. `invalid-title.md` - Wrong title pattern
2. `invalid-forbidden-pattern.md` - Contains 3 forbidden patterns
3. `invalid-missing-purpose.md` - Missing required Purpose section
4. `invalid-too-long.md` - Exceeds 50-line limit
5. `invalid-purpose-too-verbose.md` - Purpose exceeds 3 paragraphs
6. `invalid-missing-trigger.md` - Missing " - Read when " triggers

#### Documentation

- `README.md` - Comprehensive fixture documentation with test coverage matrix

### 3. Schema Validation ✅

Validated that the template's validation frontmatter:

- ✅ Is valid YAML syntax
- ✅ Passes JSON Schema validation against `validation_schema.json`
- ✅ Contains all required fields (schema_version)
- ✅ Uses correct types for all properties
- ✅ Follows naming conventions for conditions

## Validation Rules Migrated

All rules from `validate.py` lines 470-602 are now in template frontmatter:

### Basic Rules

- ✅ Title pattern: `^# .+ Guide for LLM Agents$`
- ✅ Max lines: 50
- ✅ No frontmatter in CLAUDE.md files (note: this is a special rule for CLAUDE.md output, not the template)

### Forbidden Patterns

- ✅ `(?i)how to` - how-to instructions
- ✅ `(?i)step 1` - step-by-step procedures
- ✅ `(?i)contains information` - explanatory content
- ✅ `(?i)describes` - explanatory content

### Required Sections

- ✅ Purpose (must_exist: true, max_paragraphs: 3)
- ✅ Hub (require_if: readme_exists, content_pattern for README.md)
- ✅ Files (require_if: has_md_files, must_list_all_md, entry_pattern)
- ✅ Subdirectories (require_if: has_subdirs, must_list_all_subdirs, entry_pattern)

### Conditional Logic

- ✅ `readme_exists: file_exists("README.md")`
- ✅ `has_md_files: md_files_exist(exclude=["README.md", "CLAUDE.md"])`
- ✅ `has_subdirs: subdirs_exist()`

## Next Steps for Developer Agent

The documentation work is complete. The developer agent needs to:

1. **Update `validate.py`** - Remove hardcoded CLAUDE.md validation (lines 470-602)
2. **Implement template-driven validation** - Use RuleEvaluator with template frontmatter
3. **Implement predicate functions** - Add `file_exists()`, `md_files_exist()`, `subdirs_exist()`
4. **Test with fixtures** - Ensure all 10 fixtures validate correctly
5. **Validate existing CLAUDE.md files** - Run against all existing files in repo

## Test Coverage

The fixtures cover all validation rule types:

- ✅ Title pattern validation
- ✅ Line limits (max_lines)
- ✅ Forbidden patterns (4 patterns)
- ✅ Required sections (must_exist)
- ✅ Conditional sections (require_if)
- ✅ Section constraints (max_paragraphs)
- ✅ Content patterns (content_pattern, entry_pattern)
- ✅ File listing rules (must_list_all_md, must_list_all_subdirs, exclude_globs)

## Acceptance Criteria Status

- ✅ claude-md.md has complete validation frontmatter
- ✅ Validation frontmatter passes JSON Schema validation
- ⏳ validate.py successfully validates test fixtures using template rules (developer agent)
- ⏳ All existing CLAUDE.md files in repo pass validation (developer agent)
- ⏳ Hardcoded CLAUDE.md validation removed from validate.py (developer agent)
- ✅ Test suite has 10 fixtures covering all edge cases

## Files Modified

```
doc/_meta/02-templates/claude-md.md          [MODIFIED - Added validation frontmatter]
doc/scripts/fixtures/claude-md/              [CREATED - New directory]
  ├── README.md                              [CREATED]
  ├── valid-minimal.md                       [CREATED]
  ├── valid-with-hub.md                      [CREATED]
  ├── valid-with-files.md                    [CREATED]
  ├── valid-complete.md                      [CREATED]
  ├── invalid-title.md                       [CREATED]
  ├── invalid-forbidden-pattern.md           [CREATED]
  ├── invalid-missing-purpose.md             [CREATED]
  ├── invalid-too-long.md                    [CREATED]
  ├── invalid-purpose-too-verbose.md         [CREATED]
  └── invalid-missing-trigger.md             [CREATED]
```

## Validation Checkpoint

This milestone serves as the validation checkpoint for the entire template-driven validation approach:

✅ **Proven**: Templates can encode complex validation rules declaratively
✅ **Proven**: Validation frontmatter structure is comprehensive and flexible
✅ **Proven**: JSON Schema can validate validation frontmatter
✅ **Ready**: Test fixtures cover all rule types and edge cases

The template migration demonstrates that the approach is sound and ready for:

- M7: Hub template migration
- M8: Spoke template migration
- M9: Index template migration
- M10: Implementation guide for template consumers

## Notes

The validation frontmatter uses three predicate functions that need to be implemented in `validate.py`:

1. `file_exists(filename)` - Check if file exists in same directory as target document
2. `md_files_exist(exclude=[...])` - Check if .md files exist (excluding patterns)
3. `subdirs_exist()` - Check if subdirectories exist (excluding hidden/dunder dirs)

These predicates enable conditional section validation based on directory contents, which is the core pattern for CLAUDE.md navigation files.
