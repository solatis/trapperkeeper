---
doc_type: spoke
status: active
date_updated: 2025-11-07
primary_category: documentation
hub_document: doc/_meta/01-standards/README.md
tags:
  - standards
  - claude-md
  - navigation
maintainer: Documentation Team
---

# CLAUDE.md Format Standard

## Purpose

This standard defines requirements, structure, and validation rules for CLAUDE.md navigation files used throughout Trapperkeeper documentation. CLAUDE.md files serve as fast navigation indexes for LLM agents, providing directory-level guidance on when to read specific files and subdirectories.

## Decision

All documentation directories MUST include a CLAUDE.md file following the **fast-index navigation pattern** with explicit opening triggers for each file and subdirectory.

CLAUDE.md files are pure navigation aids optimized for LLM agent discovery. They contain NO explanations, NO how-to instructions, NO philosophical rationale—only directory purpose and triggers for when to read each resource.

## Requirements

### Naming and Location

- **Filename**: MUST be exactly `CLAUDE.md` (all caps)
- **Location**: MUST be in the directory it describes (e.g., `doc/03-data/CLAUDE.md` describes `doc/03-data/`)
- **Coverage**: Every numbered category directory (01-principles, 02-architecture, etc.) MUST have a CLAUDE.md
- **\_meta/ Coverage**: Every subdirectory in `_meta/` (standards/, templates/, governance/, tooling/) MUST have a CLAUDE.md

### Frontmatter

**CRITICAL**: CLAUDE.md files NEVER use YAML frontmatter. Frontmatter is only for README.md (hubs) and .md files (spokes/indexes).

### Required Structure

CLAUDE.md MUST contain EXACTLY these sections in order:

1. **Title**: `# [Directory Name] Guide for LLM Agents`
2. **Purpose**: 1-2 sentences describing directory content
3. **Hub** (if hub exists): Single entry for README.md with trigger
4. **Files** (if files exist): List of `*.md` files with triggers
5. **Subdirectories** (if subdirs exist): List of subdirectories with triggers

Omit sections 3-5 if they don't apply (e.g., directory has no hub, no files, or no subdirs).

### Section Specifications

#### Title

```markdown
# [Directory Name] Guide for LLM Agents
```

- MUST use exact format: directory name + "Guide for LLM Agents"
- Examples: "Security Guide for LLM Agents", "Standards Guide for LLM Agents"

#### Purpose

```markdown
## Purpose

[1-2 sentence description of what this directory contains and what it's for]
```

- MUST be 1-2 sentences (no more)
- States WHAT directory contains, not WHY or HOW
- No explanations of concepts or architectural decisions

**Good examples:**

- "Standards for validation architecture, authentication patterns, and rule expression processing."
- "Templates providing copy-paste starting points with inline guidance for creating documentation."

**Bad examples:**

- ❌ "This directory is important because it contains all the security-related documentation which helps developers understand our security architecture..." (too verbose)
- ❌ "Contains stuff about validation" (too vague)

#### Hub Section

```markdown
## Hub

**`README.md`** - Read when [specific trigger/threshold]
```

- MUST list README.md if it exists in this directory
- Trigger MUST be specific and actionable
- Use format: "Read when [doing X]" or "Read when [need Y]"

**Good triggers:**

- "Read when understanding validation strategy across 4 layers"
- "Read when implementing or troubleshooting security controls"

**Bad triggers:**

- ❌ "Important overview document" (not a trigger)
- ❌ "Contains information about..." (explanatory, not trigger)

#### Files Section

```markdown
## Files

**`file-name.md`** - Read when [specific trigger]
**`another-file.md`** - Read when [specific trigger]
```

- List ALL `.md` files in this directory (except README.md and CLAUDE.md)
- One file per line
- Alphabetical order recommended but not required
- Each file MUST have a trigger

#### Subdirectories Section

```markdown
## Subdirectories

**`subdir-name/`** - Read when [specific trigger]
```

- List ONLY immediate subdirectories (1 level deep)
- Do NOT list files within subdirectories (that's the subdirectory's CLAUDE.md responsibility)
- Include trailing `/` to indicate directory
- Each subdir MUST have a trigger

### Trigger Requirements

Triggers MUST be:

- **Specific**: Clear threshold or condition for when to read
- **Actionable**: Describes a task, need, or situation
- **Concise**: 5-15 words typically

Triggers MUST NOT:

- Explain what the file contains (that's duplication)
- Provide how-to instructions (that's in the file itself)
- Include philosophical rationale (that's in the file itself)
- Be vague (e.g., "useful information")

**Good trigger patterns:**

- "Read when [implementing X]"
- "Read when [understanding Y]"
- "Read when [debugging Z]"
- "Read when [N+ threshold met]" (e.g., "Read when 3+ related documents need consolidation")
- "Read when [validation fails with X error]"

**Bad trigger patterns:**

- "Contains information about X" (explanatory)
- "Describes how to do Y" (explanatory)
- "Important file" (vague)
- "Step 1: Do X, Step 2: Do Y..." (how-to instructions)

### Forbidden Content

CLAUDE.md files MUST NOT contain:

- **Detailed concept explanations**: Don't explain what hub-and-spoke is; say "read X to understand hub-and-spoke"
- **How-to instructions or procedures**: No step-by-step guides; only triggers pointing to files with procedures
- **Duplicate information from target files**: Never duplicate what's in referenced files
- **Philosophical rationale or design decisions**: No "why we chose X"; only "read Y when choosing X"
- **Multi-level subdirectory navigation**: Don't list subdirectory contents; only immediate subdirs
- **Markdown tables, code blocks, diagrams**: Pure text list format only
- **Links to external resources**: Only internal documentation references

### Length Guidelines

- **Target**: 20-40 lines total
- **Maximum**: 50 lines
- **Depth 0 (doc/CLAUDE.md)**: Can be slightly longer (up to 60 lines) due to root-level navigation needs

If exceeding maximum:

- Triggers are too verbose (make them more concise)
- Forbidden content is present (remove explanations, how-tos, rationale)
- Too many files/subdirs (consider consolidation via hub document)

## Examples

### Minimal CLAUDE.md (Directory with Hub Only)

```markdown
# Security Guide for LLM Agents

## Purpose

Security architecture, authentication strategies, encryption patterns, and threat models.

## Hub

**`README.md`** - Read when implementing or troubleshooting security controls across authentication, encryption, or transport layers
```

### Complete CLAUDE.md (Hub + Files + Subdirs)

```markdown
# Validation Guide for LLM Agents

## Purpose

Validation architecture, input sanitization standards, and validation responsibility across UI, API, runtime, and database layers.

## Hub

**`README.md`** - Read when understanding validation strategy across 4 layers

## Files

**`api-validation.md`** - Read when implementing API layer validation or security enforcement
**`database-validation.md`** - Read when implementing database constraints or data integrity checks
**`runtime-validation.md`** - Read when implementing business logic validation in SDK or server
**`ui-validation.md`** - Read when implementing web form validation or user feedback

## Subdirectories

**`examples/`** - Read when need concrete validation implementation examples
```

### CLAUDE.md for \_meta/ Directory

```markdown
# Documentation Meta-Documentation Guide for LLM Agents

## Purpose

Standards, templates, governance, and tooling for writing and maintaining Trapperkeeper documentation.

## Hub

**`README.md`** - Read when creating, updating, or maintaining any documentation

## Subdirectories

**`governance/`** - Read when maintaining hubs, resolving conflicts, or conducting quarterly reviews
**`standards/`** - Read when need requirements, quality criteria, or validation rules
**`templates/`** - Read when creating new hub, spoke, index, or CLAUDE.md files
**`tooling/`** - Read when implementing, debugging, or understanding validation automation
```

## Validation Rules

validate-claude-md checker MUST enforce:

1. **Filename**: MUST be `CLAUDE.md`
2. **No frontmatter**: File MUST NOT start with `---` YAML block
3. **Title format**: MUST match `# [Text] Guide for LLM Agents`
4. **Purpose section**: MUST exist with `## Purpose` heading
5. **Purpose length**: MUST be 1-3 paragraphs (allow 3 for root doc/CLAUDE.md)
6. **Hub section** (if README.md exists in directory): MUST list README.md with trigger
7. **Files section** (if .md files exist): MUST list all .md files except README.md and CLAUDE.md
8. **Subdirectories section** (if subdirs exist): MUST list all immediate subdirectories
9. **Trigger format**: Each file/subdir MUST have `- Read when` trigger pattern
10. **No forbidden patterns**: MUST NOT contain "how to", "step 1", "contains information", "describes"
11. **Length limit**: MUST NOT exceed 50 lines (60 for doc/CLAUDE.md)

## Coverage Requirements

**Product Documentation**:

- Every numbered category directory (01-principles, 02-architecture, 03-data, etc.) MUST have CLAUDE.md
- Subdirectories MAY have CLAUDE.md if they contain multiple files or subdirs

**Meta-Documentation**:

- doc/\_meta/ MUST have CLAUDE.md
- Every \_meta/ subdirectory (standards/, templates/, governance/, tooling/) MUST have CLAUDE.md

## Relationship to Other Documents

- **Hub documents (README.md)**: Provide strategic overview and navigation within domain
- **CLAUDE.md**: Provides fast-index navigation triggers to hub and files
- **Difference**: README.md has frontmatter and detailed content; CLAUDE.md has neither

Think of it as:

- CLAUDE.md = Fast index with opening triggers (optimized for quick LLM discovery)
- README.md (hub) = Strategic overview with detailed navigation (optimized for understanding domain)

## Maintenance

### When to Update

CLAUDE.md MUST be updated when:

- New .md file added to directory (add to Files section)
- .md file removed from directory (remove from Files section)
- New subdirectory added (add to Subdirectories section)
- Subdirectory removed (remove from Subdirectories section)
- README.md hub created (add Hub section)
- Trigger becomes stale or inaccurate (update trigger)

### Update Workflow

1. Make change to directory (add/remove file or subdir)
2. Update CLAUDE.md immediately in same PR/commit
3. Ensure triggers remain concise and actionable
4. Run `validate-claude-md` to verify compliance

## Related Documents

- **templates/claude-md.md**: Copy-paste template with inline guidance and good/bad examples
- **standards/hub-and-spoke-architecture.md**: Hub creation criteria and structure requirements
- **tooling/architecture.md**: validate-claude-md implementation specification

## Appendix: Root doc/CLAUDE.md Special Cases

The root `doc/CLAUDE.md` serves as entry point for all documentation navigation and has slightly relaxed requirements:

- **Length**: Up to 60 lines allowed (vs 50 for other CLAUDE.md files)
- **Purpose**: May be 2-3 sentences (vs 1-2 for subdirectories)
- **Scope**: Lists both numbered category directories AND special files/directories (\_meta, README.md, indexes)

This exception exists because root CLAUDE.md is the LLM agent's first navigation touchpoint and benefits from slightly more context.
