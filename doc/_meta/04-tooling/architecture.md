---
doc_type: spoke
status: active
date_created: 2025-11-06
date_updated: 2025-11-07
primary_category: documentation
hub_document: README.md
tags:
  - tooling
  - architecture
  - validation
maintainer: Documentation Team
---

# Documentation Tooling Architecture

## Purpose

This document defines the architecture, interfaces, and governance model for documentation automation tooling in Trapperkeeper. It provides design guidance for analyzer.py, validators, and CI/CD integration to ensure consistent documentation quality.

## Overview

Documentation tooling serves three primary functions:

1. **Validation**: Enforce structural and content requirements
2. **Generation**: Automate repetitive documentation tasks
3. **Maintenance**: Detect drift and staleness

All tooling is implemented in Python for consistency and maintainability.

## Core Tool: analyzer.py

### Architecture

`analyzer.py` implements a subcommand architecture pattern:

```
analyzer.py
├── validate-all           # Master validation command
├── validate-frontmatter   # YAML frontmatter validation
├── validate-hub-spoke     # Bidirectional link validation
├── validate-indexes       # Cross-cutting index validation
├── validate-links         # Broken link detection
├── generate-index         # Cross-cutting index generation
└── check-hub-freshness    # Hub-spoke synchronization check
```

### Design Principles

1. **Fail Fast**: Exit with non-zero status on first error when using `--strict`
2. **Actionable Errors**: Error messages include file path, line number, and fix suggestion
3. **Incremental Validation**: Each subcommand can run independently
4. **Composable**: `validate-all` orchestrates individual validators
5. **Configuration-Driven**: Validation rules in external config files, not hardcoded

### Subcommand Specifications

#### validate-frontmatter

**Purpose**: Validate YAML frontmatter against JSON Schema definitions

**Usage**:

```bash
python doc/scripts/analyzer.py validate-frontmatter [--strict] [paths...]
```

**Algorithm**:

1. Load schema files (frontmatter-spoke.schema.json, frontmatter-hub.schema.json)
2. For each document:
   - Extract YAML frontmatter
   - Determine doc_type (hub vs spoke vs index)
   - Validate against appropriate schema
   - Check conditional requirements (e.g., superseded_by when status=superseded)
3. Report validation errors with file path and field name
4. Exit 0 if all valid, 1 if errors found

**Error Output Format**:

```
[ERROR] doc/security/authentication.md:
  - Missing required field: primary_category
  - Field 'status' has invalid value 'archived' (allowed: draft, active, deprecated, superseded)
```

**Dependencies**:

- Python jsonschema library
- PyYAML for frontmatter extraction

#### validate-hub-spoke

**Purpose**: Ensure bidirectional linking between hubs and spokes

**Usage**:

```bash
python doc/scripts/analyzer.py validate-hub-spoke [--strict]
```

**Algorithm**:

1. Identify all hub documents (doc_type=hub in frontmatter)
2. For each hub:
   - Extract consolidated_spokes list from frontmatter
   - Verify each spoke file exists
   - Load each spoke and extract hub_document field
   - Verify spoke references this hub
3. Identify orphaned spokes (references hub that doesn't list it)
4. Report broken references

**Error Output Format**:

```
[ERROR] Hub doc/security/README.md lists spoke 'doc/security/missing.md' that does not exist

[ERROR] Spoke doc/security/tls.md references hub doc/security/README.md but hub does not list it in consolidated_spokes

[WARNING] Hub doc/performance/README.md has 12 spokes (consider splitting, threshold is 10)
```

**Exit Codes**:

- 0: All references valid
- 1: Broken references found
- 2: Warnings only (exit 0 unless --strict)

#### validate-indexes

**Purpose**: Validate cross-cutting index completeness and accuracy

**Usage**:

```bash
python doc/scripts/analyzer.py validate-indexes [--strict]
```

**Algorithm**:

1. Identify all documents with cross_cutting frontmatter
2. For each cross-cutting concern (security, performance, validation, observability, error-handling):
   - Load corresponding index document
   - Extract all document links from index
   - Compare with documents having that concern in frontmatter
   - Report missing documents
3. Validate index structure (required sections present)
4. Check review dates not expired (>3 months)

**Error Output Format**:

```
[ERROR] Security index missing documents:
  - doc/security/authentication.md (has cross_cutting: security in frontmatter)
  - doc/api/authentication-sensor-api.md (has cross_cutting: security in frontmatter)

[WARNING] Performance index last reviewed 2024-08-01 (>3 months ago, next review due)

[ERROR] Validation index missing required section: Domain Coverage Matrix
```

#### validate-links

**Purpose**: Detect broken internal links and cross-references

**Usage**:

```bash
python doc/scripts/analyzer.py validate-links [--strict] [--external]
```

**Algorithm**:

1. For each markdown document:
   - Extract all links (markdown and HTML)
   - For internal links:
     - Resolve relative path
     - Verify target file exists
     - If anchor (#section), verify section exists
   - For external links (optional, slow):
     - HTTP HEAD request with 5s timeout
     - Report 404s
2. Build link graph for visualization

**Error Output Format**:

```
[ERROR] doc/security/README.md:45
  Broken link: [Validation Hub](doc/validation/README.md)
  Target does not exist

[ERROR] doc/performance/README.md:78
  Broken anchor: [Cost Model](#cost-model-section)
  Section heading not found in target document

[WARNING] doc/external-reference.md:12
  External link returned 404: https://example.com/missing-page
```

**Performance**: Internal link validation should complete in <5 seconds for 100 documents

#### generate-index

**Purpose**: Generate baseline cross-cutting index from frontmatter

**Usage**:

```bash
python doc/scripts/analyzer.py generate-index --concern security [--output path]
```

**Algorithm**:

1. Scan all documents with `cross_cutting: security` in frontmatter
2. Group by primary_category for Domain Coverage Matrix
3. Extract document title (first H1 heading)
4. Extract first paragraph as description
5. Generate markdown following template structure:
   - Frontmatter with doc_type=index
   - Purpose section (generic)
   - Quick Reference table (grouped by category)
   - Core Concepts sections (one per category)
   - Domain Coverage Matrix
   - Maintenance Notes
6. Preserve manual sections marked with:

   ```markdown
   <!-- BEGIN MANUAL SECTION -->

   [content]

   <!-- END MANUAL SECTION -->
   ```

**Output**:

- Writes to stdout or specified file
- DOES NOT overwrite existing index without --force flag
- Merges with existing manual sections

**Limitations**:

- Generated content is baseline only
- Primary maintainer must add narrative, patterns, relationships
- Automated updates preserve manual customizations

#### check-hub-freshness

**Purpose**: Detect hub-spoke synchronization drift

**Usage**:

```bash
python doc/scripts/analyzer.py check-hub-freshness
```

**Algorithm**:

1. For each hub document:
   - Extract date_updated from frontmatter
   - Load all consolidated spokes
   - Extract date_updated from each spoke
   - If any spoke updated more recently than hub, flag drift
2. Report potential drift with dates

**Output Format**:

```
[WARNING] Hub doc/security/README.md may be stale:
  Hub last updated: 2025-11-01
  Spoke doc/security/tls.md updated: 2025-11-05 (4 days newer)
  Spoke doc/security/api-auth.md updated: 2025-11-03 (2 days newer)

Recommendation: Review hub for synchronization with spoke changes
```

**Exit Code**: Always 0 (warnings only, not errors)

#### validate-all

**Purpose**: Run all validation checks in sequence

**Usage**:

```bash
python doc/scripts/analyzer.py validate-all [--strict]
```

**Execution Order**:

1. validate-frontmatter (foundational - structure must be valid)
2. validate-hub-spoke (depends on frontmatter)
3. validate-indexes (depends on frontmatter)
4. validate-links (can run independently but slower)
5. check-hub-freshness (informational only)

**Behavior**:

- Continues through all validators even if earlier ones fail
- Reports summary at end
- Exits with non-zero if any validator failed
- In CI/CD, this is the primary merge gate

## Tool Orchestration

### Execution Order

Critical dependencies between tools:

```
validate-frontmatter (foundational)
    ↓
validate-hub-spoke (requires valid frontmatter)
validate-indexes (requires valid frontmatter)
    ↓
validate-links (can run in parallel, slower)
    ↓
check-hub-freshness (informational)
```

### Configuration Model

Validation rules stored in `doc/scripts/config/validation-rules.yaml`:

```yaml
frontmatter:
  schemas:
    hub: doc/_meta/standards/frontmatter-hub.schema.json
    spoke: doc/_meta/standards/frontmatter-spoke.schema.json

hub_spoke:
  max_spokes_per_hub: 10
  warn_spokes_threshold: 8

indexes:
  review_frequency_days: 90
  required_sections:
    - Purpose
    - Quick Reference
    - Core Concepts
    - Domain Coverage Matrix
    - Maintenance Notes

links:
  timeout_seconds: 5
  retry_attempts: 2
  exclude_patterns:
    - localhost
    - 127.0.0.1
```

This separation allows updating rules without code changes.

### Implementation Constraints

#### Python Standard Library Only

The validate.py implementation uses ONLY Python standard library to ensure:

- Zero external dependencies
- Simple installation (Python 3.9+ required, no pip install)
- Portability across environments
- Easy maintenance for 5-person team

**YAML Parsing**: Manual regex-based extraction (frontmatter is simple enough)
**Schema Validation**: Manual field checking (covers 95% of validation needs)

This trade-off prioritizes simplicity and maintainability over exhaustive validation.

### Error Handling Workflow

Standardized error handling across all validators:

```python
class ValidationError:
    file_path: str
    line_number: Optional[int]
    severity: Literal["ERROR", "WARNING", "INFO"]
    message: str
    fix_suggestion: Optional[str]

def report_error(error: ValidationError):
    """Consistent error formatting"""
    output = f"[{error.severity}] {error.file_path}"
    if error.line_number:
        output += f":{error.line_number}"
    output += f"\n  {error.message}"
    if error.fix_suggestion:
        output += f"\n  Fix: {error.fix_suggestion}"
    print(output, file=sys.stderr)
```

### Version Management

**Reproducibility Requirements**:

- Pin all Python dependencies in requirements.txt
- Document Python version requirement (3.9+)
- Lock schema versions (commit schemas to repo)
- Version configuration files

**Example requirements.txt**:

```
jsonschema==4.20.0
PyYAML==6.0.1
markdown==3.5.1
lychee==0.14.0  # external link checker
```

## CI/CD Integration

### GitHub Actions Workflow

```yaml
name: Documentation Validation

on:
  pull_request:
    paths:
      - "doc/**"

jobs:
  validate-docs:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Setup Python
        uses: actions/setup-python@v4
        with:
          python-version: "3.9"

      - name: Install dependencies
        run: pip install -r doc/scripts/requirements.txt

      - name: Validate documentation
        run: |
          python doc/scripts/analyzer.py validate-all --strict

      - name: Check hub freshness
        run: |
          python doc/scripts/analyzer.py check-hub-freshness
```

### Merge Gate

Pull requests CANNOT merge unless:

1. `validate-frontmatter` passes
2. `validate-hub-spoke` passes
3. `validate-indexes` passes
4. `validate-links` passes (internal links only)

`check-hub-freshness` warnings do not block merge but create visibility.

### Performance Targets

CI/CD validation must complete quickly:

- validate-frontmatter: <2 seconds
- validate-hub-spoke: <3 seconds
- validate-indexes: <3 seconds
- validate-links (internal): <5 seconds
- **Total: <15 seconds for all validators**

If validation exceeds 30 seconds, optimize or parallelize.

## Governance Model

### Tooling Ownership

**Primary Owner**: Single designated owner for all documentation tooling

**Responsibilities**:

- Maintain analyzer.py and validators
- Fix bugs in tooling
- Add new validation rules as needed
- Update configuration files
- Document breaking changes

**For Trapperkeeper**: With 5-person team, tooling owner is likely the person who writes most documentation.

### Change Review Process

**Minor Changes** (bug fixes, error message improvements):

- Owner implements
- Any team member reviews PR
- No special approval needed

**Major Changes** (new validators, schema changes):

- Owner proposes with rationale
- Team discusses if needed
- Document in this architecture document
- Update templates/governance docs if affected
- Two team members review PR

**Schema Changes**:

- Update JSON Schema files
- Update frontmatter-reference.md
- Test against existing documents
- Phase in with warnings before errors
- Team approval required

### Maintenance SLA

**Critical Tool Bugs** (blocking merges):

- Fix within 1 business day
- Temporary workaround if needed
- Document in known issues

**Non-Critical Bugs** (false positives, incorrect warnings):

- Fix within 1 week
- Add to backlog
- Workaround documented

**Enhancement Requests**:

- Evaluate usefulness
- Prioritize based on frequency of need
- Implement when capacity available

## External Tools Integration

### Lychee Link Checker

For comprehensive external link validation:

```bash
lychee --exclude localhost --exclude 127.0.0.1 doc/**/*.md
```

**Integration**: Optional in CI/CD (slow), recommended for quarterly reviews

**Configuration**: `.lychee.toml` in repository root

### Prettier Markdown Formatting

For consistent markdown formatting:

```bash
prettier --check "doc/**/*.md"
```

**Integration**: Pre-commit hook (optional), CI/CD check (recommended)

**Configuration**: `.prettierrc` in repository root

### Tool Execution Order

When running multiple tools:

```bash
# 1. Format (modifies files)
prettier --write "doc/**/*.md"

# 2. Validate structure (fast, fails fast)
python doc/scripts/analyzer.py validate-all --strict

# 3. Check external links (slow, optional)
lychee doc/**/*.md
```

## Implementation Pseudocode

### analyzer.py Structure

```python
#!/usr/bin/env python3
"""Documentation validation and generation tooling."""

import argparse
import sys
from pathlib import Path
from typing import List, Dict

from validators import (
    FrontmatterValidator,
    HubSpokeValidator,
    IndexValidator,
    LinkValidator
)
from generators import IndexGenerator

def main():
    parser = argparse.ArgumentParser(description="Documentation tooling")
    subparsers = parser.add_subparsers(dest='command', required=True)

    # validate-frontmatter subcommand
    frontmatter = subparsers.add_parser('validate-frontmatter')
    frontmatter.add_argument('--strict', action='store_true')
    frontmatter.add_argument('paths', nargs='*')

    # validate-hub-spoke subcommand
    hub_spoke = subparsers.add_parser('validate-hub-spoke')
    hub_spoke.add_argument('--strict', action='store_true')

    # validate-indexes subcommand
    indexes = subparsers.add_parser('validate-indexes')
    indexes.add_argument('--strict', action='store_true')

    # validate-links subcommand
    links = subparsers.add_parser('validate-links')
    links.add_argument('--strict', action='store_true')
    links.add_argument('--external', action='store_true')

    # generate-index subcommand
    generate = subparsers.add_parser('generate-index')
    generate.add_argument('--concern', required=True)
    generate.add_argument('--output')
    generate.add_argument('--force', action='store_true')

    # check-hub-freshness subcommand
    freshness = subparsers.add_parser('check-hub-freshness')

    # validate-all subcommand
    validate_all = subparsers.add_parser('validate-all')
    validate_all.add_argument('--strict', action='store_true')

    args = parser.parse_args()

    # Dispatch to appropriate handler
    handlers = {
        'validate-frontmatter': handle_validate_frontmatter,
        'validate-hub-spoke': handle_validate_hub_spoke,
        'validate-indexes': handle_validate_indexes,
        'validate-links': handle_validate_links,
        'generate-index': handle_generate_index,
        'check-hub-freshness': handle_check_hub_freshness,
        'validate-all': handle_validate_all,
    }

    exit_code = handlers[args.command](args)
    sys.exit(exit_code)

def handle_validate_all(args):
    """Run all validators in sequence."""
    validators = [
        ('Frontmatter', handle_validate_frontmatter),
        ('Hub-Spoke Links', handle_validate_hub_spoke),
        ('Indexes', handle_validate_indexes),
        ('Links', handle_validate_links),
    ]

    results = {}
    for name, validator in validators:
        print(f"\n{'='*60}")
        print(f"Running {name} validation...")
        print('='*60)
        results[name] = validator(args)

    # Summary
    print(f"\n{'='*60}")
    print("VALIDATION SUMMARY")
    print('='*60)
    for name, exit_code in results.items():
        status = "✓ PASSED" if exit_code == 0 else "✗ FAILED"
        print(f"{name}: {status}")

    # Check hub freshness (informational only)
    print(f"\n{'='*60}")
    print("Hub Freshness Check (informational)")
    print('='*60)
    handle_check_hub_freshness(args)

    # Return first non-zero exit code, or 0 if all passed
    for exit_code in results.values():
        if exit_code != 0:
            return exit_code
    return 0

# ... other handler implementations
```

## Error Remediation Procedures

For each common error, document detection and fix:

### Error: Missing Required Frontmatter Field

**Detected By**: validate-frontmatter
**Error Message**: `Missing required field: primary_category`
**Root Cause**: Document missing required frontmatter field
**Fix**:

1. Open document in editor
2. Add missing field to frontmatter:
   ```yaml
   primary_category: security
   ```
3. Ensure value is from allowed enum
4. Re-run validation

### Error: Broken Hub-Spoke Reference

**Detected By**: validate-hub-spoke
**Error Message**: `Spoke doc/security/tls.md references hub that doesn't list it`
**Root Cause**: Hub and spoke frontmatter out of sync
**Fix**:

1. Open hub document
2. Add spoke to consolidated_spokes list:
   ```yaml
   consolidated_spokes:
     - doc/security/tls.md
   ```
3. Add cross-reference in hub Core Concepts section
4. Re-run validation

### Error: Index Missing Documents

**Detected By**: validate-indexes
**Error Message**: `Security index missing doc/security/new-feature.md`
**Root Cause**: New document with cross_cutting frontmatter not added to index
**Fix**:

1. Open cross-cutting index (e.g., doc/security-index.md)
2. Add document to appropriate Core Concepts subsection
3. Update Domain Coverage Matrix if needed
4. Re-run validation

## Future Enhancements

**Not Implemented in MVP**:

- Visual dependency graph generation
- Automated hub-spoke synchronization suggestions
- Natural language query interface for navigation
- Document coverage metrics dashboard
- Historical drift tracking

**Prioritize based on**:

- Frequency of need
- Impact on documentation quality
- Implementation complexity

## Related Documents

- **Standards**: `doc/_meta/standards/hub-and-spoke-architecture.md` - Hub requirements
- **Schemas**: `doc/_meta/standards/frontmatter-*.schema.json` - Validation schemas
- **Governance**: `doc/_meta/governance/hub-consolidation.md` - Hub maintenance
- **Governance**: `doc/_meta/governance/cross-cutting-index-governance.md` - Index maintenance
