# Spoke Template Test Fixtures

Test fixtures for validating spoke document template compliance.

## Valid Fixtures

### valid-minimal.md

Minimal valid spoke document meeting all requirements:

- ✓ All required frontmatter fields present
- ✓ Valid doc_type ("spoke")
- ✓ Valid status, date format
- ✓ hub_document field present and matches pattern
- ✓ Context section with hub back-reference
- ✓ Implementation sections with examples

**Purpose**: Verify validator accepts minimal valid spoke configuration.

### valid-complete.md

Complete spoke document with all optional features:

- ✓ All required frontmatter fields plus optional fields (tags, maintainer)
- ✓ Multiple detailed implementation sections
- ✓ Rich examples and code snippets
- ✓ Edge cases and limitations documented
- ✓ Complete related documents section
- ✓ Cross-references to hub and sibling spokes

**Purpose**: Verify validator accepts fully-featured spoke with comprehensive implementation detail.

## Invalid Fixtures

### invalid-frontmatter.md

Spoke document with **missing required frontmatter fields**.

- ✗ Missing `primary_category` (required)
- ✗ Missing `hub_document` (required)
- ✓ Other frontmatter fields valid

**Expected Error**: Missing required frontmatter fields.

**Purpose**: Verify frontmatter field validation.

## Validation Rules

Spoke documents must satisfy (from `spoke.md` template validation block):

**Frontmatter Required Fields**:

- `doc_type` (must equal "spoke")
- `status` (enum: draft, active, deprecated, superseded)
- `primary_category`
- `hub_document` (pattern: `.*\.md$`)

**Conditional Constraints**:

- If `status` = "superseded", then `superseded_by` field required

**Section Requirements**:

- Spoke documents have flexible section requirements (minimal structure validation)
- Content-driven organization based on implementation needs

## Differences from Hub Validation

Spokes have **more flexible structure** than hubs:

- No minimum section requirements
- No subsection count constraints
- No filename pattern (any descriptive name allowed)
- No minimum item counts (e.g., no minimum related documents)

Spokes emphasize **implementation detail freedom** while hubs enforce **strategic structure**.

## Usage

These fixtures test template-driven validation logic:

```bash
# Test spoke validation against template rules
python validate.py validate-spoke-template fixtures/spoke/valid-minimal.md
python validate.py validate-spoke-template fixtures/spoke/invalid-frontmatter.md
```

Integration with `validate.py` pending developer implementation of template-driven validation subsystem.
