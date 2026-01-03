# Hub Template Test Fixtures

Test fixtures for validating hub document template compliance.

## Valid Fixtures

### valid-minimal.md

Minimal valid hub document meeting all requirements:

- ✓ Correct filename pattern (would need to be README.md in practice)
- ✓ All required frontmatter fields present
- ✓ Minimum 3 consolidated_spokes
- ✓ Required sections: Context (2 paragraphs), Decision (3 subsections), Consequences, Related Documents
- ✓ Valid status, date format, doc_type

**Purpose**: Verify validator accepts minimal valid hub configuration.

### valid-complete.md

Complete hub document with all optional features:

- ✓ All required frontmatter fields plus optional fields (tags, maintainer, cross_cutting)
- ✓ Context section with 4 paragraphs
- ✓ Decision section with 7 concept subsections (maximum allowed)
- ✓ 5+ consolidated_spokes
- ✓ Rich cross-references and examples
- ✓ Complete consequences and related documents sections

**Purpose**: Verify validator accepts fully-featured hub with maximum allowed complexity.

## Invalid Fixtures

### invalid-not-readme.md

Hub document with **wrong filename**.

- ✗ Filename is `invalid-not-readme.md` instead of `README.md`
- ✓ All other requirements met

**Expected Error**: Filename pattern validation failure - hub documents must be named `README.md`.

**Purpose**: Verify filename pattern enforcement.

### invalid-frontmatter.md

Hub document with **missing required frontmatter fields**.

- ✗ Missing `primary_category` (required)
- ✗ Missing `consolidated_spokes` (required)
- ✓ Correct filename pattern
- ✓ Other frontmatter fields valid

**Expected Error**: Missing required frontmatter fields.

**Purpose**: Verify frontmatter field validation.

### invalid-few-spokes.md

Hub document with **too few consolidated spokes**.

- ✗ Only 2 spokes in `consolidated_spokes` list (minimum is 3)
- ✓ Correct filename pattern
- ✓ All required frontmatter fields present

**Expected Error**: Hub requires minimum 3 consolidated_spokes.

**Purpose**: Verify minimum spoke count validation.

## Validation Rules

Hub documents must satisfy (from `hub.md` template validation block):

**Filename**:

- Pattern: `^README\.md$`

**Frontmatter Required Fields**:

- `doc_type` (must equal "hub")
- `status` (enum: draft, active, deprecated, superseded)
- `primary_category`
- `consolidated_spokes` (array, min 3 items)

**Conditional Constraints**:

- If `status` = "superseded", then `superseded_by` field required

**Required Sections**:

- "Context" (2-4 paragraphs)
- "Decision" (3-7 subsections starting with `### `)
- "Consequences"
- "Related Documents"

## Usage

These fixtures test template-driven validation logic:

```bash
# Test hub validation against template rules
python validate.py validate-hub-template fixtures/hub/valid-minimal.md
python validate.py validate-hub-template fixtures/hub/invalid-frontmatter.md
```

Integration with `validate.py` pending developer implementation of template-driven validation subsystem.
