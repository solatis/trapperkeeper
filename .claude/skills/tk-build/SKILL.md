---
name: tk-build
description: Open the build stage of a unit -- per issue, in dependency order, cut the branch, land the commits its approved plan lists with their comments and document rows, run the doc-discipline checks, push and open the PR; then watch every PR's Actions runs while the owner reviews each PR in plannotator, bottom of a stack first, and post the build-stage message. Invoked by the owner after the plan-stage message; the invocation is the gate.
disable-model-invocation: true
argument-hint: "[#n ... , only when the session does not already name the unit's issues]"
---

# /tk-build -- the build stage

Run `/tk-workflow` first. An argument, when the owner gave one, is:

<issues>
$ARGUMENTS
</issues>

This stage may run in a session that has none of the seed or plan
conversation. The plans are the whole input: read each whole before its
first commit.

## The plans

The issues are the argument's, else the unit's issues named in this
session's plan-stage message. Each issue's plan is the `.plans/*-plan.md`
whose `Issue:` line names it:

!`grep -H -E -r --include='*-plan.md' '^(Issue|Branch|Base|Status):' .plans 2>/dev/null || echo none`

No plan for an issue, or two plans naming the same issue, is a stop: say
which, and the owner decides. With no argument and no plan-stage message in
this session, stop and ask for the issue numbers.

Order the issues so that every issue comes after the issue its `Base:`
names. That order is the landing order for everything below.

## Entry check

Current branch: !`git branch --show-current`

Master vs origin, commits ahead / behind (0 0 means in sync):

!`git fetch -q origin 2>/dev/null; git rev-list --left-right --count master...origin/master`

Working tree (empty means clean):

!`git status --short`

Local branches other than master:

!`git branch --format='%(refname:short)' | grep -vx master || true`

Open pull requests:

!`gh pr list --state open --json number,headRefName,baseRefName --jq '.[] | "#\(.number) \(.headRefName) -> \(.baseRefName)"' 2>&1 || true`

CI workflows in the tree (empty means no CI):

!`ls .github/workflows/ 2>/dev/null || true`

The tree is clean, master is in sync with origin, and each plan says
`Status: approved`. Anything else is the owner's decision before anything
else; a plan still at `draft` means the gate has not approved it.

Per issue, the branch decides where the build starts:

- No branch `<slug>`: the issue starts at The commits.
- The branch exists with an open PR: its commits are done; the issue
  starts at The runs and the reviews.
- The branch exists with no PR: the build landed part of it in an earlier
  session. Stop for that issue and report its `git log --oneline
  <base>..<slug>` against the plan's Commits; the owner decides.

## The commits

Run `/doc-discipline read` once, before the first code commit of the
session. Then, issue by issue in landing order:

1. Cut the branch from its base: `git checkout -b <slug> master`, which
   the entry check has shown in sync with origin, or `git checkout -b
<slug> <parent slug>` for a stacked issue, at the parent branch's current
   head.
2. Land the plan's Commits section in order, one small commit each, each
   building and passing lint and tests with the commands the touched
   component's `AGENTS.md`, How to, names, or the root `AGENTS.md` for
   repo-wide ones. The plan is followed to the letter; a deviation the code
   forces is said in the build-stage message, never made silently.

   Every commit carries its comments and document rows as the plan's
   Design and Knowledge sections specify them: the executor composes the
   wording, in the shape the root `AGENTS.md`, "Code comments", prescribes,
   and every claim the specification lists is present once, in the same
   commit as the code. A claim is written as a claim, without the plan's
   evidence brackets. A claim the code contradicts is not written; it is
   reported in the build-stage message with both versions.

   A reason that arises while building (a rejected alternative, a
   threshold, an ordering constraint) and is not in the plan is written
   where it is decided, with its evidence. One you would have to invent is
   asked through the question tool before the commit that needs it, never
   written as a guess and never left out silently.

3. After the last code commit, `/doc-discipline all <paths the branch
touched>`, and one small commit per finding.
4. `/doc-discipline check <paths the branch touched>
.plans/<slug>-plan.md`, which reconciles the code against the plan and
   reports every reason the plan promised that the code and documents do
   not state. Its "Unlanded from the plan" findings are fixed with a
   further small commit each, or reported with the reason each was
   dropped. Keep its report for the build-stage message.

   The reports of steps 3 and 4 are read in this repository's terms: a
   "Rationale unknown" finding is a question to the owner through the
   question tool before the push, and the answer lands in one further
   commit; a "Needs a human" finding is a line of the build-stage message
   and gets no commit.

5. Push and open the PR, with the github skill's "Create":

   ```
   git push -u origin <slug>
   gh pr create -B <base> -t "<PR title>" -b ""
   ```

6. With CI in the tree, start the watcher for the pushed head, in the
   background, and go on to the next issue without waiting:

   ```
   CHECKPOINT=7200 ~/.claude/skills/github/scripts/gh-watch.sh $(git rev-parse HEAD)
   ```

   Without CI, run the repository's build, lint and tests for the touched
   area and `npx prettier --check` on the touched Markdown instead, and
   keep their result for the build-stage message.

Correct: a step comment needs to say why the server refuses an empty
batch; you do not know; you ask, and the answer becomes the comment.

Incorrect: the comment says "the server refuses an empty batch for safety",
because that sounded right. Or: the plan's Rationale says the per-request
signature was rejected for a session token, no comment or `AGENTS.md` Why
says so, and the PR is opened with that reason only in the plan, which the
merge stage deletes.

## The runs and the reviews

The owner reviews the code while CI runs rather than after it. Two things
happen on their own schedule during this section, a branch getting new
commits and a watcher exiting, so both are defined here, before the review
loop in which they occur.

Restack: when a branch gets new commits and other branches of the unit
stack on it, rebase each of them onto its new head (`git rebase --onto
<parent> <old parent head> <child>`), `git push --force-with-lease`, and
start a watcher for each new child head. Their reviews have not run yet,
because reviews run bottom-up, so no review is lost. A rebase that
conflicts is stopped and reported; the owner decides.

A watcher's exit says what comes next:

| Exit | Last line                  | Next                                                                                                                                                                                                                                                                                                                               |
| ---- | -------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 0    | `FINAL <sha> passed`       | Green. Read the runs back for the build-stage message (`gh api 'repos/{owner}/{repo}/actions/runs?head_sha=<sha>'`), never from memory of having started them.                                                                                                                                                                     |
| 1    | `FINAL <sha> <conclusion>` | Red. Read the log tails. A failure the branch causes is fixed with further small commits on it, pushed, watched again, restacked, and its PR reviewed again, because the owner has not seen the fix. A failure that the same workflow also shows on master's head, or an outage, is reported with the evidence; the owner decides. |
| 2    | `NORUNS ...`               | CI is in the tree, yet nothing ran for this head: the workflow's triggers do not cover this branch or PR. Stop and report; the issue cannot end without a run.                                                                                                                                                                     |
| 3    | `STATUS ...`               | The checkpoint passed with runs still going. Relaunch the watcher with the same SHA and keep waiting.                                                                                                                                                                                                                              |
| 4    | `no repo ...`              | Stop and report.                                                                                                                                                                                                                                                                                                                   |

plannotator runs one session at a time, and a review diffs the checked-out
branch, so the reviews run one after another, in landing order, bottom of a
stack first. For each PR:

1. `git checkout <slug>`, then open the review in the background, with the
   timeout `/tk-workflow`, Tools, fixes:

   ```
   plannotator review --base <base> --json
   ```

   It blocks until the owner decides in the browser. Do nothing else while
   it runs except handle a watcher that exits, as the table above says. Its
   stdout is one JSON record; classify it by `decision` alone:
   - `approved`: the owner's yes on this PR's code. Notes in `message` are
     guidance, quoted in the build-stage message, not a change request.
     Go on to the next PR.
   - `annotated`: the owner's change requests. Address every annotation
     with further small commits on this branch, push, start a watcher for
     the new head, restack, and open this PR's review again. The watcher
     of the superseded head is reported as superseded, not waited for.
   - `dismissed`, or a non-zero exit without a decision: stop and report;
     the owner decides in the conversation.

An issue's build is over only when its reviewed head is its pushed head,
its review is `approved`, and that head is green, or, without CI, its local
checks passed on that head. Local tests passing is not the end of the stage
when CI exists, however small the change.

Correct: logging and metrics are built and pushed, metrics stacked on
logging, two watchers running. The logging review returns two annotations;
you fix them in two commits, push, start a watcher, rebase metrics onto the
new logging head, force-push it, start its watcher, and review logging
again; it returns `approved`. Then the metrics review returns `approved`.
Both new heads go green; the message names their runs and the two
superseded heads.

Incorrect: the review returns `approved` and the build-stage message is
posted while the runs are still going. Or: the logging fix is pushed and
the metrics review opens on the old stack, so the owner reviews code that
will never merge. Or: a run goes red, you fix it and wait for green without
reopening the review, so the owner merges a commit they never saw.

## The build-stage message

Exactly, per issue in landing order: the issue number and the branch; the
PR's URL and base; a summary of the change in plain words; each deviation
from the plan with the reason the code forced it; the doc-discipline
check's report, or "clean", with its "Needs a human" findings; the review's
decision with its notes quoted, if any; the run ids and their state for the
reviewed head, and any superseded heads, or the local checks that ran when
there is no CI; the command `git diff <base>...<slug>`. Then one line
saying the merge waits for `/tk-merge`.

Then wait. Do not ask whether to merge; the owner invokes `/tk-merge` when
they are ready.
