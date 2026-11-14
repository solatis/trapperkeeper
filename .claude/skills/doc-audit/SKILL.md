---
name: doc-audit
description: Audits code and documents against the repository's comment and documentation rules and returns the changes they call for as rows of a report.
context: fork
agent: doc-reader
background: false
argument-hint: "<base>...HEAD | <paths> [plan: <plan>] [focus: ...] [also: ...]"
allowed-tools: Bash(git diff:*), Bash(grep:*), Bash(python3:*)
---

# Audit comments and documents

The caller's words: $ARGUMENTS

You audit code and documents against the repository's rules for comments and
documents. You change no file. You propose each change as a row of your
report, and the session that called you decides on each row.

## The changed lines

!`git diff -U0 --no-color $0 2>/dev/null | grep -E '^(\+\+\+ |@@ )' || true`

When the scope is a range, each `+++ b/<path>` line names a changed file, and
each `@@ ... +<start>,<count> @@` line below it names changed lines in that
file as it is on disk now. When the scope is paths, you read every file in
scope whole and set this block aside.

## The step check

!`python3 "${CLAUDE_SKILL_DIR}/../doc-discipline/scripts/check_steps.py" --warn --words $ARGUMENTS 2>&1 || true`

A line whose rule is `W1`, `W2` or `W3` is a warning, and you judge whether
it calls for a row.

## The caller's words

- The scope is a range `<base>...HEAD` or one or more paths.
- `plan:` is followed by the path of the plan to reconcile against. Every
  word before `plan:`, `focus:` or `also:` belongs to the scope.
- `focus:` is followed by one or more of `placement`, `comments`, `wording`
  and `numbering`. You run only those checks. Without a focus you run every
  check.
- `also:` is followed by the paths of documents that belong to the task.
- The scope leaves out generated code, vendored code, lock files and every
  file that git ignores. For paths in scope, you still read the `AGENTS.md`
  chain from the root down to them, but you report only knowledge that
  concerns those paths.

## What you read

1. The policy: the "Documentation strategy", "Code comments" and "Prose"
   sections of the root `AGENTS.md`, and the `AGENTS.md` of the
   documentation folder that the policy names.
2. `${CLAUDE_SKILL_DIR}/../doc-discipline/SKILL.md`, whole.
3. `${CLAUDE_SKILL_DIR}/../doc-discipline/narrative.md` when the scope holds
   code or tests. You apply it to each function in scope.
4. `${CLAUDE_SKILL_DIR}/../doc-discipline/placement.md` when the scope holds
   Markdown. You apply it, and the rules of the documentation folder's
   `AGENTS.md`, to each document in scope.
5. The documents of the change: each `AGENTS.md` from the root down to each
   path in scope, each decision record that the code in scope cites, the
   policy sections above, and the paths after `also:`.
6. Each changed function and each changed section of a document, whole.

## The checks

- `placement`: where each piece of knowledge lives, by `placement.md`.
- `comments`: the level of each function and its comments, by
  `narrative.md`, with the cut pass and the gaps.
- `wording`: each sentence of a comment or document in scope against the
  "Prose" rules of the policy.
- `numbering`: the step check above. Each of its findings is a row. The
  step check above runs without the policy's deny patterns, so you read the
  patterns from the policy and search the files in scope for each of them.

When a plan is named, follow "Reconciling against a plan document" in the
guide. A reason that the code, a test, a document or a commit does not
support is a question under "Rationale unknown", and you write no guess at
it. Each row names a file, a line and one change or one question.

## Your answer

Your answer starts with the Run line of the guide's "Report" section, which
repeats the caller's words verbatim. The plan tables of `narrative.md` and
`placement.md` follow, and then the other sections of the report. Add nothing
before the Run line or after the report.

You are done when you have run each check of the focus over every file in
scope and written one row for each finding.
