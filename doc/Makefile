.PHONY: help validate-all validate-format validate-lint validate-frontmatter validate-semantic

# Find all docs except _meta/ and doc/CLAUDE.md
DOCS := $(shell find . -name '*.md' ! -path './_meta/*' ! -name 'CLAUDE.md' -o -path './[0-9]*/**/*.md')

help:
	@echo "Trapperkeeper Documentation Validation"
	@echo ""
	@echo "Targets:"
	@echo "  validate-all          Run all validation layers"
	@echo "  validate-format       Layer 1: Prettier formatting"
	@echo "  validate-lint         Layer 2: Markdownlint syntax"
	@echo "  validate-frontmatter  Layer 3: Frontmatter schema"
	@echo "  validate-semantic     Layer 4: Semantic validation"
	@echo ""
	@echo "Excludes: _meta/ directory and doc/CLAUDE.md"

validate-format:
	@echo "=== Layer 1: Prettier Formatting ==="
	npx -y prettier --write $(DOCS)

validate-lint:
	@echo "=== Layer 2: Markdownlint Syntax ==="
	npx -y markdownlint-cli2 $(DOCS)

validate-frontmatter:
	@echo "=== Layer 3: Frontmatter Schema ==="
	python3 scripts/validate.py validate-frontmatter

validate-semantic:
	@echo "=== Layer 4: Semantic Validation ==="
	python3 scripts/validate.py validate-all

validate-all: validate-format validate-lint validate-frontmatter validate-semantic
	@echo ""
	@echo "=========================================="
	@echo "All validation layers passed!"
	@echo "=========================================="
