---
doc_type: hub
status: active
primary_category: documentation
consolidated_spokes:
  - validate.py
  - rule_evaluator.py
  - predicates.py
  - markdown_parser.py
  - test_rule_evaluator.py
  - test_predicates.py
  - test_markdown_parser.py
  - test_conditional_rules.py
  - test_validation_schema.py
---

# Documentation Validation Tools

## Overview

This directory contains validation automation for Trapperkeeper documentation. The tooling ensures structural integrity, hub-spoke relationships, and template compliance across all documentation files.

## Quick Start

### Installation

No external dependencies required. Python 3.9+ with standard library only:

```bash
python3 --version  # Verify Python 3.9 or higher
```

For template validation (requires PyYAML):

```bash
pip install PyYAML
```

### Basic Usage

```bash
# Validate all documentation
python3 doc/scripts/validate.py validate-all

# Validate specific aspects
python3 doc/scripts/validate.py validate-frontmatter
python3 doc/scripts/validate.py validate-hub-spoke
python3 doc/scripts/validate.py validate-claude-md
```

## Tools

### validate.py

Main validation tool with subcommand architecture. Provides:

- **validate-frontmatter**: Check YAML frontmatter fields against schema
- **validate-hub-spoke**: Verify bidirectional hub-spoke relationships
- **validate-meta-directories**: Ensure \_meta/ subdirectories have README.md hubs
- **validate-claude-md**: Validate CLAUDE.md files against template rules
- **validate-hub**: Validate hub documents (README.md) against template
- **validate-spoke**: Validate spoke documents against template
- **validate-cross-cutting-index**: Validate cross-cutting indexes
- **validate-redirect-stub**: Validate redirect stub documents
- **check-complexity**: Check template validation rule complexity
- **validate-all**: Run all validators in sequence

See `doc/_meta/04-tooling/architecture.md` for detailed specifications.

### Supporting Modules

- **rule_evaluator.py**: Template-driven validation engine
- **predicates.py**: Validation DSL predicates (section_exists, forbidden_text, etc.)
- **markdown_parser.py**: Markdown structure parser (sections, headings, lists)

### Test Files

- **test_rule_evaluator.py**: Rule evaluator tests
- **test_predicates.py**: Predicate logic tests
- **test_markdown_parser.py**: Parser tests
- **test_conditional_rules.py**: Conditional validation tests
- **test_validation_schema.py**: Schema validation tests

## Common Workflows

### Pre-Commit Validation

```bash
# Quick validation before committing
python3 doc/scripts/validate.py validate-frontmatter
python3 doc/scripts/validate.py validate-hub-spoke
```

### Full CI/CD Validation

```bash
# Complete validation suite (as run in CI/CD)
python3 doc/scripts/validate.py validate-all
```

### Troubleshooting Validation Errors

```bash
# Show effective validation rules for CLAUDE.md
python3 doc/scripts/validate.py validate-claude-md --show-effective-rules

# Show effective validation rules for hub documents
python3 doc/scripts/validate.py validate-hub --show-effective-rules

# Show effective validation rules for spoke documents
python3 doc/scripts/validate.py validate-spoke --show-effective-rules
```

## Error Examples

### Missing Frontmatter Field

```
[ERROR] doc/security/authentication.md: Missing required field: primary_category
```

**Fix**: Add missing field to frontmatter YAML block.

### Broken Hub-Spoke Reference

```
[ERROR] Spoke doc/security/tls.md: hub_document points to README.md, expected reference to doc/security/README.md
```

**Fix**: Update hub_document field in spoke to match hub location.

### Section Validation Failure

```
[ERROR] doc/06-security/README.md: Missing required section: Core Concepts
```

**Fix**: Add required section heading to document.

## Architecture

The validation system uses:

1. **Template-driven validation**: Rules defined in template frontmatter (doc/\_meta/02-templates/)
2. **DSL-based rules**: Declarative validation expressions (see validation-dsl-reference.md)
3. **Composable validators**: Independent validators that can run separately or together
4. **Zero external dependencies**: Uses only Python standard library (except PyYAML for templates)

See `doc/_meta/04-tooling/architecture.md` for complete architecture documentation.

## Performance

Validation targets for 100-document corpus:

- validate-frontmatter: <2 seconds
- validate-hub-spoke: <3 seconds
- validate-claude-md: <2 seconds
- validate-all: <15 seconds

## Development

### Running Tests

```bash
# Run all tests
python3 -m pytest doc/scripts/

# Run specific test suite
python3 -m pytest doc/scripts/test_rule_evaluator.py
python3 -m pytest doc/scripts/test_predicates.py
```

### Adding New Validators

1. Add validation logic to validate.py
2. Register subcommand in main() argparse setup
3. Add handler function to handlers dictionary
4. Update validate-all sequence if needed
5. Document in doc/\_meta/04-tooling/architecture.md

### Modifying Validation Rules

Template validation rules are in `doc/_meta/02-templates/*.md` frontmatter. See `doc/_meta/04-tooling/validation-dsl-reference.md` for DSL syntax.

## Related Documentation

- **Architecture**: `doc/_meta/04-tooling/architecture.md` - Complete tooling architecture
- **DSL Reference**: `doc/_meta/04-tooling/validation-dsl-reference.md` - Validation rule syntax
- **Templates**: `doc/_meta/02-templates/` - Template files with validation rules
- **Standards**: `doc/_meta/01-standards/` - Frontmatter schema and documentation standards
