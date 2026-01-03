---
doc_type: template
template_for: claude-md
status: active
date_updated: 2025-11-07
primary_category: documentation
hub_document: doc/_meta/02-templates/README.md
tags:
  - templates
  - claude-md
maintainer: Documentation Team

validation:
  schema_version: 1

  conditions:
    readme_exists: file_exists("README.md")
    has_md_files: md_files_exist(exclude=["README.md", "CLAUDE.md"])
    has_subdirs: subdirs_exist()

  title_pattern: "^# .+ Guide for LLM Agents$"
  max_lines: 50

  forbidden:
    - pattern: "(?i)how to"
      reason: "how-to instructions"
      severity: error
    - pattern: "(?i)step 1"
      reason: "step-by-step procedures"
      severity: error
    - pattern: "(?i)contains information"
      reason: "explanatory content"
      severity: error
    - pattern: "(?i)describes"
      reason: "explanatory content"
      severity: error

  required_sections:
    - name: "Purpose"
      must_exist: true
      max_paragraphs: 3

    - name: "Hub"
      require_if: readme_exists
      content_pattern: '^\*\*`README\.md`\*\* - Read when'

    - name: "Files"
      require_if: has_md_files
      files_rules:
        must_list_all_md: true
        exclude_globs: ["README.md", "CLAUDE.md"]
        entry_pattern: '^\*\*`.+\.md`\*\* - Read when'

    - name: "Subdirectories"
      require_if: has_subdirs
      files_rules:
        must_list_all_subdirs: true
        entry_pattern: '^\*\*`.+/`\*\* - Read when'
---

# [Directory Name] Guide for LLM Agents

## Purpose

[1-2 sentence purpose of this directory. What content does it contain? What is it for?]

## Hub

**`README.md`** - Read when [specific trigger/threshold for when to read the hub document]

## Files

**`file-name.md`** - Read when [specific trigger/threshold]
**`another-file.md`** - Read when [specific trigger/threshold]

## Subdirectories

**`subdirectory-name/`** - Read when [specific trigger/threshold for when to navigate into this subdirectory]

---

## Template Usage Notes

**DELETE THIS SECTION** when using this template. The following guidelines apply:

### Structure Requirements

1. **Purpose**: Exactly 1-2 sentences describing directory content and intent
2. **Hub**: List README.md (if exists) with a clear trigger
3. **Files**: List each .md file in this directory with specific opening triggers
4. **Subdirectories**: List only immediate subdirectories (1 level), not their contents

### Content Rules

**REQUIRED**: Clear, specific triggers/thresholds for when to read each file
**FORBIDDEN**:

- Detailed explanations of concepts
- How-to instructions or step-by-step procedures
- Duplicate information from target files
- Philosophical rationale or design decisions
- Multi-level subdirectory navigation (don't list subdirectory contents)

### Trigger Examples

**Good triggers** (specific, actionable thresholds):

- `README.md` - Read when understanding validation strategy across 4 layers
- `input-sanitization.md` - Read when implementing OWASP injection prevention
- `frontmatter-reference.md` - Read when adding frontmatter fields to documents
- `hub-consolidation.md` - Read when 3+ related documents need consolidation
- `architecture.md` - Read when implementing or debugging validation tooling

**Bad triggers** (vague, explanatory, duplicate content):

- ❌ "Contains information about validation" (vague)
- ❌ "Describes the 4-layer validation matrix which includes..." (duplicates content)
- ❌ "Important file for understanding security" (not actionable)
- ❌ "This document explains why we chose OWASP..." (philosophical rationale)
- ❌ "Follow these steps to create a hub: 1. ..." (how-to instructions)

### Structure Enforcement

CLAUDE.md must contain EXACTLY these sections in order:

1. Title: `# [Directory Name] Guide for LLM Agents`
2. Purpose: 1-2 sentences
3. Hub: `README.md` entry (if hub exists in this directory)
4. Files: List of `*.md` files with triggers (if files exist)
5. Subdirectories: List of subdirs with triggers (if subdirs exist)

Omit sections 3-5 if they don't apply (e.g., no hub, no files, no subdirs).

### Length Guideline

Target ~20-40 lines total. If exceeding 50 lines, triggers are too verbose or forbidden content is present.
