---
name: tk-plan
description: Open the plan stage of the current unit -- create the GitHub issues that do not exist yet, write one uncommitted plan per issue in .plans/, and yield all of them through one plannotator gate. Invoked by the owner after the seed report; the invocation is the gate.
disable-model-invocation: true
argument-hint: "[#n ... , only when resuming plans written in another session]"
---

# /tk-plan -- the plan stage

Run `/tk-workflow` first. This stage opens on this invocation and only this
invocation. It writes plans and GitHub issues; it touches no tracked file,
creates no branch and makes no commit.

## Entry check

Current branch: !`git branch --show-current`

Master vs origin, commits ahead / behind (0 0 means in sync):

!`git fetch -q origin 2>/dev/null; git rev-list --left-right --count master...origin/master`

Working tree (empty means clean):

!`git status --short`

Plans on this machine:

!`grep -H -E -r --include='*-plan.md' '^(Issue|Branch|Base|Status):' .plans 2>/dev/null || echo none`

Issues named in the argument (empty means the seed report's):

<issues>
$ARGUMENTS
</issues>

A seed report for this unit is in the conversation; its Brief, Issues,
Decisions, Open questions and Proposed commits are the inputs here. Without
one, stop and say so, unless the argument names issues whose plans exist
with `Status: draft`: then the plans are the input, and the stage resumes
at the gate (step 5). A plan with `Status: approved` for an issue in the
unit is already through this stage: say so and leave it out.

## Step by step

1. Create the issues the seed report marks "new", one at a time, each with
   its title verbatim from the report:

   ```
   gh issue create -t "<Issue title>" -b "<the issue's paragraph>" [-m "<active milestone title>"]
   ```

   The body is one paragraph in plain words: what the issue delivers and
   why, taken from the report's Brief and Issues sections, plus one line
   `Depends on #<n>` for a stacked issue. `-m` is set when the unit serves
   the active milestone. Note each new issue's number.

2. `/doc-discipline read`.
3. Read `plan-shape.md` and write one plan per issue,
   `.plans/<slug>-plan.md`, in that shape, with `Status: draft`. Create
   `.plans/` if it is missing; it is gitignored, so nothing here shows in
   `git status`.
4. Check the plans. A why you cannot verify, or a function you cannot place
   under the comment rules, is a question to the owner through the question
   tool, asked now; the answer goes into the plan. What the owner leaves
   unanswered is an open question of the plan-stage message.

   Read each plan as the executor would: for each commit, can each comment
   be composed from its specification alone, with every claim traceable to
   its evidence, and can each of its choices be defended from the Rationale
   alone? Each "no" is a gap in the Design, Rationale or Knowledge section,
   filled now. Then read the plans side by side: a decision two plans rest
   on is a row in both, worded the same; a stacked plan's Base names the
   other plan's slug, and its commits assume only what the other plan's
   commits deliver.

5. Yield all of the unit's draft plans through one review session, never
   through a typed question:

   ```
   plannotator annotate .plans/<slug-1>-plan.md .plans/<slug-2>-plan.md --gate --json
   ```

   One session, the plans in landing order, one decision for all of them.
   Run it in the background, with the timeout `/tk-workflow`, Tools, fixes;
   do nothing while it runs.
   Its stdout is one JSON decision:
   - `approved` (with optional `feedback`): the owner's yes on these plans,
     and on the plans alone. Change each plan's `Status:` line from `draft`
     to `approved`, so a session seeded fresh reads them as approved. Then
     post the plan-stage message and wait; the build stage opens when the
     owner invokes `/tk-build`.
   - `annotated`: the owner's redirect. Fold every annotation into the plan
     it points at, and gate all of the unit's plans again from step 5.
     Never start code on an annotated plan.
   - `dismissed`, or a non-zero exit without a decision: stop and report;
     the owner decides in the conversation.

The plan-stage message, posted on approval, is exactly: one line per issue
with its number, URL, slug and plan path, in landing order; the owner's
`feedback` quoted, if any, with how the build will honor it; the open
questions the plans recorded, each with the recommendation the approval has
now settled; one line saying the build waits for `/tk-build`.

Correct: you create issue #14 for the one new issue, read doc-discipline,
write `.plans/server-logging-plan.md` and `.plans/metrics-endpoint-plan.md`,
check them, run the gate on both, wait; the decision is `approved`; you set
both statuses, post the message and wait; the owner invokes `/tk-build`.

Incorrect: the gate returns `approved` and you create the first branch
because the plans are approved. Or: you gate each plan in its own session,
so the owner approves the metrics plan before seeing the logging plan it
stacks on. Or: you ask "Approve the plans?" in the conversation instead of
running the gate. Or: the gate returns annotations and you start code while
addressing them.
