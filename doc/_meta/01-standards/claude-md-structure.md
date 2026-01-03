---
doc_type: spoke
status: active
date_created: 2025-11-06
date_updated: 2025-11-07
primary_category: documentation
hub_document: doc/_meta/01-standards/README.md
tags:
  - standards
  - claude-md
  - structure
maintainer: Documentation Team
---

# CLAUDE.md Structure Standard

## Purpose

This document defines the canonical structure and requirements for CLAUDE.md files throughout the Trapperkeeper documentation system.

CLAUDE.md files are navigation aids that route LLM agents to the right documentation using action-based triggers.

## Core Principle

CRITICAL: CLAUDE.md files are navigation aids ONLY. NEVER duplicate content from target files.

They provide clear triggers for when to read specific files, then rely on those files to contain the actual information. This minimizes indirection and keeps agents focused on finding the right document quickly.

**FORBIDDEN in CLAUDE.md files:**

- Content summaries or abstracts
- Implementation details from target files
- Step-by-step procedures (point to files instead)
- Section listings from target files

## When to Create CLAUDE.md

**ALWAYS create for:**

- Directories with 3+ documentation files
- Documentation hubs requiring navigation
- Directories with specialized file types (standards, templates, tools)

**NEVER create for:**

- Directories with only README.md
- Code directories (create README.md instead)
- Directories with single file

## Standard Structure for Subdirectory CLAUDE.md

All subdirectory CLAUDE.md files MUST use EXACTLY this structure. Any deviation is non-compliant:

```markdown
# [Directory Name] Directory

## Purpose

[One sentence describing what this directory contains]

## Files

**`filename.md`** - Read when:

- [Clear trigger 1]
- [Clear trigger 2]
- [Clear trigger N]

[Repeat for each file in directory]
```

**Structure Requirements:**

- **Purpose section**: Exactly one sentence
- **Files section**: Every file with "Read when:" or "Use when:" triggers

**Content Requirements:**

- **Minimal**: NEVER duplicate content from target files
- **Trigger clarity**: Every trigger is actionable and situation-based

**Scope Requirements:**

- **No formatting commands**: Only in `tooling/linters.md`
- **No philosophy**: Only in `doc/_meta/CLAUDE.md`

## Exceptions to Standard Structure

### doc/CLAUDE.md (Entry Point)

**Purpose:** Orient LLM agents to the overall documentation system and route them to specific areas.

**May include:**

- Brief architecture overview (hub-and-spoke pattern)
- Hub creation decision triggers
- Minimal frontmatter examples
- Pointers to `_meta/` directory

**Role:** Main entry point for LLM agents navigating documentation.

### doc/\_meta/CLAUDE.md (Meta Hub)

**Purpose:** Guide LLM agents on how to write and maintain CLAUDE.md files themselves.

**May include:**

- Philosophy statement about CLAUDE.md as navigation aids
- Directory structure overview
- Quick navigation to subdirectories
- Reference to `tooling/linters.md` for formatting

**Role:** Meta-level guidance for the CLAUDE.md system itself.

## Verification Checklist

**CRITICAL (Must Fix):**

- [ ] **Purpose**: Single sentence only
- [ ] **Files**: All files listed with triggers
- [ ] **No duplication**: Zero content from target files

**IMPORTANT (Should Fix):**

- [ ] **Trigger quality**: Specific and situation-based
- [ ] **No formatting**: Commands removed (only in linters.md)
- [ ] **No philosophy**: Philosophy removed (only in doc/\_meta/CLAUDE.md)

**NICE TO HAVE:**

- [ ] **Up to date**: File list matches directory contents
- [ ] Consistent terminology throughout
- [ ] Alphabetical file ordering

## Content Duplication Check

Before finalizing any CLAUDE.md, verify:

1. Open CLAUDE.md and target file side-by-side
2. Check each sentence in CLAUDE.md
3. If sentence appears in target file → DELETE from CLAUDE.md
4. If sentence summarizes target content → DELETE from CLAUDE.md
5. If sentence helps navigate to file → KEEP in CLAUDE.md

ONLY navigation triggers belong in CLAUDE.md.

## Trigger Guidelines

Triggers are the most critical component of CLAUDE.md files. They determine when agents read files.

### Good Triggers (Specific, Actionable)

- "Read when: Creating any hub document"
- "Use when: Need example of proper hub structure"
- "Read when: Adding frontmatter to any document"
- "Check when: Questioning hub structure or quality requirements"
- "Read when: Implementing analyzer.py or validation scripts"

### Poor Triggers (Vague, Non-actionable)

- ❌ "Contains information about hubs"
- ❌ "Describes the hub pattern"
- ❌ "Everything about frontmatter"
- ❌ "Read for details"
- ❌ "Important document"

**Key difference:** Good triggers describe the SITUATION that requires reading the file, not the CONTENTS of the file.

### Transformation Pattern

Convert content-based descriptions to situation-based triggers:

❌ "Contains authentication logic" → ✅ "Read when: Implementing user login"
❌ "Has API schemas" → ✅ "Read when: Adding new API endpoint"
❌ "Explains rate limiting" → ✅ "Use when: Configuring request throttling"

**Pattern**: Convert "has/contains X" to "when [agent needs X for Y]"

### Trigger Quality Requirements

Each trigger MUST be:

- **Actionable**: Agent knows exactly when to use it
- **Situation-based**: Describes WHEN not WHAT
- **Specific**: No ambiguous words (details, information, overview)
- **Unique**: Different from other triggers in same file

## DO and DON'T

### DO:

- Provide clear thresholds: "Read X when creating hub" not "X contains sections 1-5..."
- Use action triggers: "Use when:", "Read when:", "Check when:"
- Keep it minimal: Purpose + file list + triggers
- Update when adding/removing files in directory
- Make triggers situation-based (when agent needs something)

### DON'T:

- Duplicate file contents → **Impact**: Agents read CLAUDE.md instead of authoritative source
- Provide detailed explanations → **Impact**: Maintenance burden, staleness, confusion
- Create long procedural guides → **Impact**: Defeats navigation purpose
- Explain what's obvious from file names → **Impact**: Token waste, noise
- Include formatting commands → **Impact**: Command fragmentation across files
- Add philosophy or meta-commentary → **Impact**: Dilutes navigation focus

## Hierarchical Structure

CLAUDE.md files form a navigation tree:

```
doc/CLAUDE.md                          # Entry point
    ↓ references
doc/_meta/CLAUDE.md                    # Meta hub
    ↓ references
doc/_meta/standards/CLAUDE.md          # Standards navigation
doc/_meta/templates/CLAUDE.md          # Templates navigation
doc/_meta/governance/CLAUDE.md         # Governance navigation
doc/_meta/tooling/CLAUDE.md            # Tooling navigation
```

Each level provides navigation to the next, with clear triggers for when to descend.

## Maintenance Decision Tree

**Question 1**: Did file list in directory change?

- File added/removed/renamed → UPDATE: Files section
- Otherwise → Continue to Question 2

**Question 2**: Did file PURPOSE change?

- Triggers no longer match file use → UPDATE: Triggers
- Otherwise → Continue to Question 3

**Question 3**: Did directory PURPOSE change?

- Directory role changed → UPDATE: Purpose section
- Otherwise → NO UPDATE NEEDED

**NEVER update for:**

- File content changes (CLAUDE.md describes WHEN, not WHAT)
- File length changes
- Implementation detail changes
- Examples in target file updated
- File reformatting

## Integration with README.md

Documentation maintains both navigation systems:

- **CLAUDE.md**: Concise navigation for LLM agents (hierarchical, action-triggered)
- **README.md**: Narrative overviews for humans (contextual, explanatory)

Both serve different audiences and purposes. CLAUDE.md is NOT a replacement for README.md.

**FORBIDDEN:**

- Converting README.md to CLAUDE.md (audiences differ)
- Duplicating README content in CLAUDE.md
- Writing CLAUDE.md content for humans
- Making CLAUDE.md narrative or explanatory

## Examples

### Compliant Subdirectory CLAUDE.md

```markdown
# Standards Directory

## Purpose

Formal requirements, schemas, and quality criteria for documentation.

## Files

**`hub-and-spoke-architecture.md`** - Read when:

- Creating first hub document
- Questioning hub structure or quality requirements
- Understanding 3-click navigation metric

**`frontmatter-reference.md`** - Read when:

- Adding frontmatter to any document
- Validating field names or values
- Checking required vs optional fields
```

### Non-Compliant CLAUDE.md (Too Much Content)

```markdown
# Standards Directory

## Purpose

This directory contains formal requirements, schemas, and quality criteria that
define how documentation should be structured and validated.

## Background

We created this directory because we needed a place for standards...

## Files

**`hub-and-spoke-architecture.md`**

This file contains the complete standard for hub-and-spoke documentation pattern.
It has 5 main sections:

1. Hub Requirements
2. Spoke Requirements
3. Quality Criteria
4. Navigation Metrics
5. Maintenance Procedures

Read this when you need any information about hubs.

## Formatting

\`\`\`bash
npx -y prettier --write doc/\_meta/standards/{file}
\`\`\`
```

**Problems:**

- ❌ Purpose is too long (multiple sentences)
- ❌ Includes "Background" section (unnecessary)
- ❌ Duplicates file contents (listing sections)
- ❌ Vague trigger ("any information about hubs")
- ❌ Includes formatting section (should be in linters.md)

### Fixing Non-Compliant CLAUDE.md

**Step 1: Remove content duplication**

```markdown
# Standards Directory

## Purpose

Formal requirements, schemas, and quality criteria for documentation.

## Files

**`hub-and-spoke-architecture.md`**

Read this when you need any information about hubs.
```

**Step 2: Sharpen triggers**

```markdown
**`hub-and-spoke-architecture.md`** - Read when:

- Creating first hub document
- Questioning hub structure or quality requirements
- Understanding 3-click navigation metric
```

**Step 3: Verify no duplication remains**

Run content duplication check:

1. Open CLAUDE.md and hub-and-spoke-architecture.md
2. Verify no sentences from target appear in CLAUDE.md
3. Verify triggers describe situations, not contents

**Result: Compliant CLAUDE.md** (shown in first example)

## Related Standards

- `hub-and-spoke-architecture.md` - Defines hub and spoke document structure
- `frontmatter-reference.md` - Defines frontmatter metadata fields
- `doc/_meta/tooling/linters.md` - Formatting and linting commands
- `doc/_meta/governance/documentation-evolution-principle.md` - Evolution principles

## Questions and Validation

**How do I know if my CLAUDE.md is compliant?**

→ Run through the Verification Checklist (CRITICAL items first)

**Can I add additional sections beyond Purpose and Files?**

→ Only for the two documented exceptions (`doc/CLAUDE.md` and `doc/_meta/**/CLAUDE.md`)

**Can I include examples in CLAUDE.md?**

→ NO. Point to files that contain examples instead.

**My directory has 20+ files. Do I list them all?**

→ YES, or consider if the directory structure needs reorganization

**Can I group files by category in CLAUDE.md?**

→ NO. Keep it simple: Purpose + flat file list. Directory structure provides grouping.

**My file has obvious purpose from its name. Do I still need triggers?**

→ YES. Triggers describe WHEN to read, which is never obvious from name alone.
