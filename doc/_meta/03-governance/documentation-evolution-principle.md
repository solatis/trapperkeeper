---
doc_type: spoke
status: active
date_created: 2025-11-06
date_updated: 2025-11-07
primary_category: documentation
hub_document: README.md
tags:
  - governance
  - evolution
  - principles
maintainer: Documentation Team
---

# Documentation Evolution Principle

## Core Principle

**When making any decision, design choice, or architectural change: rewrite documentation as if that decision was always part of the original design.**

## Rationale

Documentation with visible evolution history (e.g., "we used to do X, now we do Y") creates cognitive load and confusion:

- Readers must distinguish between current and historical approaches
- LLM agents may reference deprecated information
- Navigation requires understanding timeline, not just structure
- Maintenance burden increases with each historical reference

**Clean slate approach**: When a decision changes, documentation reflects only the current state. Git provides the history; documentation provides the present truth.

## Application

### When to Rewrite

Rewrite documentation when:

- **Architectural decision changes** → Rewrite affected docs as if new approach was original
- **Naming or terminology changes** → Update all references consistently
- **Structure reorganization** → Rewrite navigation as if structure was always this way
- **Process or governance changes** → Document only current process
- **Technology changes** → Remove references to old technology entirely

### What to Remove

Remove these patterns:

- ❌ "Previously, we used X, but now we use Y"
- ❌ "This replaces the old approach of..."
- ❌ "Historical note: we used to..."
- ❌ "Updated from previous version..."
- ❌ Revision history sections or version numbers in documentation

### What to Keep

Retain in documentation:

- ✅ Current architecture and decisions
- ✅ Rationale for current approach (not comparison to past)
- ✅ Future evolution possibilities
- ✅ References to external standards or prior art

Keep in git:

- ✅ Complete commit history showing evolution
- ✅ Git blame for understanding when/why changes occurred
- ✅ PR discussions documenting decision context

## Examples

### Example 1: CLAUDE.md Strategy

**Bad approach** (historical baggage):

> "We previously used README.md for navigation, but have now adopted hierarchical CLAUDE.md files for LLM agents while keeping README.md for humans."

**Good approach** (current state):

> "Documentation uses hierarchical CLAUDE.md files for LLM agent navigation. Each directory contains a CLAUDE.md providing concise navigation aids with clear triggers for when to read specific files."

### Example 2: Hub Structure

**Bad approach** (evolutionary trail):

> "Documentation used to be flat files. We migrated to hub-and-spoke structure where hubs consolidate related documents."

**Good approach** (clean slate):

> "Documentation uses hub-and-spoke architecture. Hubs consolidate 3+ related documents, providing strategic overview and navigation to focused implementation documents (spokes)."

### Example 3: Technology Change

**Bad approach** (historical comparison):

> "We previously used PostgreSQL for events but switched to JSONL file storage for simplicity."

**Good approach** (current rationale):

> "Events are stored in JSONL format for operational simplicity, easy backups, and seamless tool integration. See data/event-schema-storage.md for details."

## Special Cases

### When History Matters

Rare cases where historical context is valuable:

- **Compatibility notes**: "Field X is deprecated but supported for v1.x clients"
- **Migration guidance**: "Migrating from v1 to v2" (in migration guide, not core docs)
- **External commitments**: "API v1 supported until 2026" (commitment, not history)

In these cases, frame as **forward-looking constraints**, not backward-looking history.

### Appendices and Metadata

Hub documents use `consolidated_spokes` frontmatter field for tracking which documents are consolidated. This is metadata for tooling, not user-visible historical narrative.

## Implementation Process

When updating documentation:

1. **Make decision** (architecture, naming, structure)
2. **Identify affected documents** (grep, analyzer.py, manual review)
3. **Rewrite each document** from scratch with current state
4. **Remove historical references** to old approach
5. **Update cross-references** consistently
6. **Commit with clear message** explaining the change (in git, not docs)

## Validation

Check documentation rewrite quality:

- [ ] No "previously", "used to", "old approach" language
- [ ] No version numbers or revision history sections
- [ ] Current approach described as if always this way
- [ ] Rationale explains current choice (not comparison to past)
- [ ] All cross-references updated consistently
- [ ] Git commit message documents the change context

## Relationship to Other Principles

This principle supports:

- **Simplicity**: Simpler docs without historical baggage
- **Clarity**: Single source of truth for current state
- **Maintainability**: Less to update when things change again
- **LLM-friendly**: No ambiguity about current vs historical approach

## Version

**Created**: 2025-11-06
**Status**: Active

Git provides version history. This document describes current principle.
