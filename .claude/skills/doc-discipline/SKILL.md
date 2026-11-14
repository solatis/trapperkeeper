---
name: doc-discipline
description: The guide for applying the comment and documentation rules that the repository's policy defines. It says where a piece of knowledge lives, how comments inside functions are written, which checks run after a change, and how existing code and documents are brought under the rules. Load it once, before writing code, a plan or a document.
---

# Documentation discipline

This skill is a guide. It says how to apply the repository's rules for
comments and documents, and it shows examples. The policy is the
"Documentation strategy", "Code comments" and "Prose" sections of the root
`AGENTS.md`, plus the `AGENTS.md` of the documentation folder that the
"Documentation strategy" section names (`doc/` or `docs/`). Read them first.
If anything here disagrees with them, they win; say so in the report. This
file and its references carry no policy of their own. The shape and the sizes
of comments, and the patterns the step check denies, come from the policy, so
the same copy serves every repository.

## Files

| File                      | Read it                                                                         |
| ------------------------- | ------------------------------------------------------------------------------- |
| `narrative.md`            | before you write or change code, or the sketches of a plan                      |
| `placement.md`            | before you write or move a document, or a comment that holds knowledge          |
| `checks.md`               | after a change is complete, before it leaves the session                        |
| `scripts/check_steps.py`  | the step check: it checks walkthrough numbers, back-references and denied marks |
| `scripts/change_parts.py` | the part list: it splits a change into parts for `/doc-read`                    |

The step check runs in one of four forms:

```
python3 <skill dir>/scripts/check_steps.py [--deny REGEX]... [--warn] <path>...
python3 <skill dir>/scripts/check_steps.py [--deny REGEX]... --git-range <base>...<rev> [--markdown-too]
python3 <skill dir>/scripts/check_steps.py --markdown <file.md>
python3 <skill dir>/scripts/check_steps.py --words <any words>
```

Pass one `--deny` for each pattern the policy lists. The last output line is
`check_steps: <n> findings in <m> files`. Exit status 0 means no finding, 1
means findings, and 2 means a usage or git error.

## The two checks that run in a subagent

Two companion skills run in a new subagent of their own. The subagent sees
none of the conversation, and its last message comes back to the caller.

- `/doc-read <base>...HEAD <part>` takes the range of a change and the number
  of one of its parts, and nothing else. A change is read in parts of about
  300 changed lines, one reader per part. A reader who has not seen the change
  reads the changed files as they are on disk. It returns one row for each
  line where it stopped, each function whose plan it had to rebuild, each
  comment that told it nothing, each statement it believes false, and each
  term or statement in a document it could not place.
- `/doc-audit <scope> [plan: <plan>] [focus: ...] [also: ...]` takes a range
  or paths. It may also take a plan to reconcile against, a focus that limits
  the checks, and the paths of documents that belong to the task. It returns
  the report of the "Report" section below, with each change it proposes as
  a row.

The caller of `/doc-read` passes the range and the part number alone, because
the reader is useful only while it does not know what the author knows. The
caller of `/doc-audit` adds facts about the task, and never its view of the
result.

## Who writes

The session that owns a change writes its comments and documents. It writes
each reason from evidence it can name: a line of code, a test, a document it
moves or a commit. It asks the owner for a reason it has no evidence for. An
agent that the session starts reports and writes nothing. Both companion
skills run on an agent that has no tool that writes, so they cannot change a
file.

## Hard limits

1. Comments and documentation files only. When existing code is repaired,
   never change what code does: no renames, no reordering, no refactors, no
   formatting sweeps. If code should change (a function that needs
   splitting, a misleading name, dead code), put it in the report.
2. Never invent rationale. A comment may only state what you verified from the
   code, the tests, the docs you are moving, or git history. If the "why" is
   unknown, write the "what" and ask the owner for the reason. `/doc-audit`
   lists the gap under "Rationale unknown" as a question. Every "why" you
   intend to write names its evidence in the plan table of `narrative.md` or
   `placement.md`: a `file:line`, a test, the passage being moved, or a
   commit. The evidence stays in the plan; it does not go into the comment.
3. A wrong comment is worse than none. Verify every existing comment you touch
   against the code. Fix or delete stale ones, and never leave one you know
   is wrong. A comment that reads as a patch on an earlier version, or that
   mentions a plan, a review, an issue or the conversation, counts as wrong
   even when each clause is true. It is rewritten whole from the code.
4. Code that needs a comment gets it. A missing note at a line where the
   reader stops is as much a defect as a note that repeats its line. A
   function that qualifies for a walkthrough under the policy gets one,
   whether or not any reader stopped in it. Deleting a comment that fails the
   cut pass in `narrative.md` is as much the job as writing a missing one.
5. ASCII only. Match the comment syntax, doc-comment convention and line width
   of the surrounding file.
6. Moving is cut-and-paste, not copy. After a move the knowledge exists once,
   at the most local place, and every other mention is a pointer or is gone.

## Repairing existing code and documents

A session that is asked to bring existing code and documents under the rules
works as follows:

1. Read the policy files. If the root `AGENTS.md` has no "Documentation
   strategy" section, stop and say so; there is no policy to enforce.
2. Run `/doc-audit <paths>` for one component directory at a time, leaf
   directories first. Several `/doc-audit` calls for independent components
   may run side by side. Keep each report as it came back.
3. Apply each report row by row. Write a reason only from the evidence its
   row names, or from evidence you find yourself. Ask the owner for each
   reason that has no evidence, and write that comment after the answer
   arrives. A row you do not apply is declined with its reason.
4. Verify. After editing Markdown, run `npx prettier --write` on the touched
   files. Do not run a code formatter. Review `git diff` hunk by hunk: every
   changed line in a source file must be a comment line. Revert any hunk that
   changes a non-comment token and redo it by hand. Run the step check over
   the changed files and fix each finding until it reports none. Run the
   build or tests when that is cheap.
5. Follow `checks.md` over the commits of the repair.
6. Report, in the form of the "Report" section, with the rows applied and
   declined.

## Reconciling against a plan document

A plan document says what the comments and documents were going to be,
written while the reasons were still in context; the code says what they
became. The gap between the two is where reasons get lost, because the plan
is deleted once its work lands. `/doc-audit` follows these steps when its
caller names a plan:

1. Collect every promise the plan makes about prose: each claim in the
   sketches of its design section, and each row of its rationale table with
   the landing place its knowledge section gives it (an ADR, an `AGENTS.md`
   row, a code comment, or `plan only`).
2. Skip the `plan only` rows. The plan drops them on purpose.
3. Hold every other promise to the cut pass in `narrative.md` before
   looking for it. A promise that fails the cut pass is not written, even
   when the plan asks for it: it repeats the code, it names a rejected
   alternative no reader of that line would propose, or it records the
   conversation. List it under "Dropped on purpose".
4. For each remaining promise, find where it landed. A changed wording is
   fine when the claim survives. A claim that is gone is a finding.
5. Each finding becomes one row of the narrative or placement plan table,
   with the plan as its evidence for each reason, so the session that applies
   the report writes the missing reason where the plan said it would go. A
   promise the code contradicts (the design changed while building) is not
   written. It goes under "Needs a human" with both versions quoted.
6. Report the findings under "Unlanded from the plan", one line each: the
   promise, where the plan said it would land, what is there instead.

## Report

Keep the report short, and list its rows. `/doc-audit` writes the Run line
first, then its plan tables, then each other section as proposed changes. A
session that repairs code writes each section as changes made.

- **Run**: one line that repeats the caller's words verbatim, then the
  scope, the last line of the step check, and the number of functions at
  each level (bare, notes, walkthrough).
- **Moved**: from -> to, one line each.
- **Commented**: functions whose comments were written or rewritten.
- **Removed**: stale, duplicate, historical or noise comments and doc
  passages.
- **Discoverability**: `AGENTS.md` Map rows added or changed.
- **Wording**: sentences in comments or documents that break the "Prose" rules
  of the policy, each with the rule it breaks.
- **Rationale unknown**: places where the why could not be verified, each as
  a question for the owner.
- **Unlanded from the plan**: reasons the plan document promised that no
  comment or document states; "none" when a plan was given and all landed.
- **Dropped on purpose**: plan promises that failed the cut pass, one line
  each with the defect.
- **Needs a human**: code-level findings outside this skill's limits (split
  candidates, misleading names, dead or commented-out code, doc budget
  overruns that cannot be fixed by moving).
