---
doc_type: spoke
status: active
date_updated: 2025-11-07
primary_category: documentation
hub_document: doc/_meta/04-tooling/README.md
tags:
  - tooling
  - formatting
  - linting
  - prettier
  - markdownlint
maintainer: Documentation Team
---

# Formatting and Linting Tools

## Purpose

Single source of truth for all markdown formatting and linting commands used in Trapperkeeper documentation.

## Prettier (Required)

After editing ANY markdown file:

```bash
npx -y prettier --write {file}
```

## Markdownlint (Required)

After editing ANY markdown file:

```bash
npx -y markdownlint-cli2 {file}
```

## When to Run

**Always run both tools** immediately after creating or editing any .md file

## Configuration Files

### .prettierrc.yaml

Location: `doc/.prettierrc.yaml`

Purpose: Defines formatting rules for prettier (line width, indentation, etc.)

### .markdownlint.yaml

Location: `doc/.markdownlint.yaml`

Purpose: Defines linting rules for markdownlint (which rules to enforce/ignore)

**DO NOT** modify these files without approval - formatting consistency is critical.

## Running All Validations

### Complete Validation Pipeline

From the `doc/` directory:

```bash
make validate-all
```

This runs all 4 validation layers in order:

```
1. Prettier (formatting)  → npx -y prettier --check
2. Markdownlint (syntax)  → npx -y markdownlint-cli2
3. Frontmatter (schema)   → python3 scripts/validate.py validate-frontmatter
4. Semantic (architecture) → python3 scripts/validate.py validate-all
```

### Individual Layers

Run specific validation layers:

```bash
make validate-format        # Layer 1: Prettier
make validate-lint          # Layer 2: Markdownlint
make validate-frontmatter   # Layer 3: Frontmatter
make validate-semantic      # Layer 4: Semantic
```

### File Scope

**Validated**: All `doc/**/*.md` files

**Excluded**:

- `doc/_meta/**/*` (meta-documentation)
- `doc/CLAUDE.md` (navigation file)

### Exit Codes

- `0`: Validation passed
- `1`: Validation errors found
- `2`: Warnings only (not currently used)
