---
name: tk-build
description: Open the build stage of a unit -- per issue, in dependency order, cut the branch, land the commits its approved plan lists with their comments and document rows, run the comment checks (the step check, /doc-read and /doc-audit), push and open the PR; then watch every PR's Actions runs while the owner reviews each PR in the Inbox, bottom of a stack first, and post the build-stage message. Invoked by the owner after the plan-stage message; the invocation is the gate.
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

!`grep -H -E -r --include='*-plan.md' '^(Issue|Branch|Base|Thread|Status):' .plans 2>/dev/null || echo none`

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
else; a plan still at `draft` means the owner has not approved it.

Per issue, the branch decides where the build starts:

- No branch `<slug>`: the issue starts at The commits.
- The branch exists with an open PR: its commits are done; the issue
  starts at The runs and the reviews.
- The branch exists with no PR: the build landed part of it in an earlier
  session. Stop for that issue and report its `git log --oneline
  <base>..<slug>` against the plan's Commits; the owner decides.

## The commits

Load the doc-discipline skill once, before the first code commit of the
session, and read its `narrative.md` and `placement.md`. Then, issue by
issue in landing order:

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
   evidence brackets. A bracket that names the owner and a date is
   evidence too, and it reaches no comment. A bracket that names a record
   becomes `ADR-<number>` where the comment needs the pointer. A claim the
   code contradicts is not written; it is
   reported in the build-stage message with both versions. Where a commit
   changes code that already has comments, the blocks it touches are
   rewritten whole from the new code, as the root `AGENTS.md`, "Code
   comments", prescribes. They are never patched with another clause.

   The plan's claims are the least the code carries. A line that turns
   out to make the reader stop, and that the plan did not foresee, gets its
   note in the same commit as the code. The note gives a reason, such as
   why a threshold, an ordering constraint or a constant is there, or it
   says what a dense line achieves. A rejected alternative goes into a note
   only where a reader of that line would propose it. A function that
   comes into being during the build is commented under the same rules as
   a sketched one, although the plan has no sketch of it. Each of these
   comments is written from evidence you can name. A reason you would have
   to invent is asked through the question tool before the commit that
   needs it, never written as a guess and never left out silently.

   Steps 3 to 9 check the comments after the last code commit. They follow
   the doc-discipline skill's `checks.md` with this repository's commands.
   Run each step as written, through the command or the skill it names, and
   save its output where the step says. The call to `/doc-read` holds the
   range and the part number and no other word. Reading your own work
   against the skill's text replaces none of these steps: it leaves no saved
   file, and a step without its saved file counts as not run. The issue
   reaches the push only when every step has run.

   You write every comment and document of the issue, from evidence you can
   name. `/doc-read` and `/doc-audit` each run in a subagent of their own
   on the agent `doc-reader`, which has no tool that writes. They report,
   and you decide what changes.

   The commits of steps 3 to 9 take two forms. A rename or a restructuring
   is a `refactor` commit of its own. Comments and documents land in one
   `docs` commit per file. Each of these commits builds and passes lint and
   tests like every other commit.

3. Run the step check over every file the branch changed, code and
   Markdown:

   ```
   python3 .claude/skills/doc-discipline/scripts/check_steps.py --deny 'doc/adr/' --deny '\(owner,' --git-range <base>...HEAD --markdown-too
   ```

   The check covers the whole of each changed file, so it also reports
   functions in the old shape that the branch did not touch. Fix every
   finding, and run the check again until its last line reports 0
   findings.

4. Reader round 1. List the parts of the change, with the plan's `Base:`
   branch as `<base>`:

   ```
   python3 .claude/skills/doc-discipline/scripts/change_parts.py <base>...HEAD
   ```

   Then run `/doc-read <base>...HEAD <part>` once for each part. The calls
   may run side by side. The skill gives the range and the part number to a
   reader that has seen neither the plan nor this conversation, and it
   returns the reader's answer. Save the answers verbatim, in the order of
   the parts, as `.plans/<slug>-read-1.md` before you close any stop. This
   step makes no commit.

5. Note the head as `<closed>` with `git rev-parse HEAD`. Then close every
   stop of the answers. Test the four ways in this order, and take the first
   that fits:
   - A rename or a restructuring answers the question, when the code can
     say it for itself.
   - A comment written from evidence you can name: a `path:line`, a test,
     a Rationale row of the plan or an answer of the owner. The comment
     follows the root `AGENTS.md`, "Code comments", and it answers the
     question without copying its words.
   - A question to the owner through the question tool, when no evidence
     gives the reason. Step 8 sends it.
   - A decline with its reason, when the question asks what the language
     or its standard library does, or when the line already answers it.

   A statement the reader believed false is fixed, or it stays with the
   line that shows it true as the reason. Below the answers of the readers,
   without changing them, append one line per stop to
   `.plans/<slug>-read-1.md`. Each stop gets one of these lines:

   ```
   <path>:<line> | rename <sha>
   <path>:<line> | comment <sha>
   <path>:<line> | question
   <path>:<line> | decline: <reason>
   ```

6. Reader round 2, only when step 5 changed comments, documents or code.
   List the parts of `<closed>...HEAD` as step 4 does, and run
   `/doc-read <closed>...HEAD <part>` for each part, so that new readers read
   what step 5 changed. They are not told what the readers of round 1 said.
   Save their answers verbatim, in the order of the parts, as
   `.plans/<slug>-read-2.md`, close their stops as step 5 says, and append
   the closing lines to that file. There is no third round.

7. The audit. Run `/doc-audit <base>...HEAD plan: .plans/<slug>-plan.md`.
   When the plan's Knowledge section names a document that the branch did
   not change, add `also:` and its path. The call holds no `focus:`, so
   every check of the audit runs. Save the report verbatim as
   `.plans/<slug>-audit.md`. Apply or decline each finding yourself, and
   append one line per finding below the report:

   ```
   <section>: <finding> | applied <sha>
   <section>: <finding> | declined: <reason>
   ```

   A proposal to cut a note that closed a reader's stop is declined, with
   that stop as the reason. An "Unlanded from the plan" finding lands in a
   commit, or it is declined with the reason it was dropped. A "Rationale
   unknown" finding is a question to the owner. "Dropped on purpose" and
   "Needs a human" findings get no commit.

8. Send the questions of steps 5 to 7 together, as one Inbox message for
   the issue in the unit's thread, one question block per question, as
   `/tk-workflow`, The Inbox, says. When a review waits in the thread, end
   your turn and send the message after the owner answers that review,
   because the owner can mark only the latest message. After you send it,
   end your turn until the answers arrive. Each answer lands in the `docs`
   commit of the file it concerns. With no question, skip this step.

9. Run the step check of step 3 again, fix each finding in one more commit,
   and repeat until its last line reports 0 findings.

10. Push and open the PR, with the github skill's "Create":

    ```
    git push -u origin <slug>
    gh pr create -B <base> -t "<PR title>" -b ""
    ```

11. With CI in the tree, start the watcher for the pushed head, in the
    background, and go on to the next issue without waiting:

    ```
    CHECKPOINT=7200 ~/.claude/skills/github/scripts/gh-watch.sh $(git rev-parse HEAD)
    ```

    Without CI, run the repository's build, lint and tests for the touched
    area and `npx prettier --check` on the touched Markdown instead, and
    keep their result for the build-stage message.

Correct: the note at a guard needs to say why the server refuses an empty
batch. You do not know, so you ask, and the answer becomes the note.

Correct: after the last code commit, the step check reports two findings in
one file, and you fix both in one commit. The change has two parts, and
their two readers stop at nine lines. You save their answers, write six notes
from evidence, rename one binding, ask the owner about one threshold, and
decline one stop because it asks what `?` does. The reader of round 2 reads
the six notes and the rename and stops nowhere. The audit's report is saved,
two findings are applied and one is declined, and the last step check
reports 0 findings. The guide's "Comment
checks" section shows each of these from the saved files.

Incorrect: the comment says "the server refuses an empty batch for safety",
because that sounded right. Or: the plan lands the rejection of the
per-request signature in an ADR, the PR is opened without that ADR, and the
reason dies with the plan, which the merge stage deletes. Or: every
walkthrough item names an alternative the plan rejected, and the code reads
like the planning session. Or: the build lands the note claims the plan
sketched for two guards of a parser, and the closures and tuple patterns of
its helpers stay bare, because the plan did not name them. The claims are
the least the code carries. Or: you read `checks.md` and do the cold read
and the audit from its text, without calling `/doc-read` and `/doc-audit`.
The guide says "reader: no stops, audit: clean", and
`.plans/<slug>-read-1.md` does not exist, so nothing shows the owner that no
reader ran. Or: you call `/doc-read <base>...HEAD 1 the parser helpers are
dense`. The reader finds its stops because it does not know what you know,
so the call holds the range and the part number alone. Or: you start a
subagent to write the comments for the reader's stops. It does not know the
reasons, so it writes examples, byte offsets and paraphrases of the code, and
some of them are false.

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

The owner can mark only the latest message of the unit's thread, so one
review waits at a time. The reviews run one after another, in landing
order, bottom of a stack first. For each PR:

1. Write a guided review of the PR's pushed head and send it, as
   `/tk-workflow`, The Inbox, says:
   - `get_guide_brief`, then the patch `git diff <base>...<slug>`, which
     shows only what this PR adds over its base.
   - Each chapter's overview is in this order, and a label with nothing
     under it is left out. One or two sentences: what changed. "You
     decided:" one line per owner decision the chapter rests on, the
     decision and the date, no argument. "Proposed by the build:" one line
     per proposal the chapter rests on, the choice, the alternative that
     lost, why, and the line in the diff that carries it; every proposal
     row of the plan appears under this label in exactly one chapter.
     "Read closely:" one to three lines, each naming a spot in the diff and
     what the comment or test there should carry, so the owner's eye lands
     where the comments and the tests are judged; the cue aims, it does
     not judge. One decision per sentence, the decision before its reason.
   - `submit_guide` with that exact patch, in the unit's thread. The body
     names the PR's URL and the head's SHA, because the owner's "Looks good" approves
     that head and no other. Then what changed since the last guide of
     this PR when there was one, then each deviation from the plan with
     the reason the code forced it and the chapter it lands in.
   - Last in the body, a section "Comment checks", built from the files
     `.plans/<slug>-read-1.md`, `.plans/<slug>-read-2.md` and
     `.plans/<slug>-audit.md` and from a run of the step check of The
     commits, step 3, at the pushed head:
     - the last line of the step check
     - the number of parts that each reader round read
     - every stop of each reader round, with how it was closed
     - every decline, with its reason
     - every statement a reader believed false, with what was done
     - the audit's first line, which repeats the call, and its findings,
       each as applied or declined.

     A file that does not exist is named as missing, and its check as not
     run.

   Then end your turn. A watcher that exits wakes the session too: handle
   it as the table above says, report it in the conversation, and end your
   turn again. The owner's answer is one of three, by The Inbox's table:
   - "Looks good" on the guide's message: the owner's yes on this PR's head. Go on
     to the next PR.
   - A reply: the owner's change requests, also when its words sound like
     a yes. Address every annotation and every point of the reply with
     further small commits on this branch, run the step check of The
     commits, step 3, and fix its findings until it reports 0 findings.
     Then push, start a watcher for the new head, restack, and send a new
     guide for the new head from step 1, with `reply_to` set to the
     reply's id. The watcher of the superseded head is reported as
     superseded, not waited for.
   - The thread resolved without a "Looks good": stop and report. The owner decides
     in the conversation.

An issue's build is over only when its reviewed head is its pushed head,
the owner marked its guide "Looks good", and that head is green, or, without CI, its local
checks passed on that head. Local tests passing is not the end of the stage
when CI exists, however small the change.

A service the build starts for local tests, such as a database container,
is named `tk-<what>`. The build removes it (`docker rm -f <name>`) once every
issue's build is over. A build that stops early leaves its services running
and names them in its report.

Correct: logging and metrics are built and pushed, metrics stacked on
logging, two watchers running. You send the logging guide; the owner
replies with two annotations. You fix them in two commits, push, start a
watcher, rebase metrics onto the new logging head, force-push it, start its
watcher, and send a new logging guide; the owner marks it "Looks good". Then you send
the metrics guide, and the owner marks it "Looks good". Both new heads go green; the
message names their runs and the two superseded heads.

Incorrect: a chapter overview stacks six decisions in one paragraph, each
with its defense, so the owner cannot tell which ones they made and asks
"are you saying that...". Or: the owner marks a guide "Looks good" and the build-stage message is
posted while the runs are still going. Or: the logging fix is pushed and
the metrics guide is sent on the old stack, so the owner reviews code that
will never merge. Or: both guides go out at once, and the owner can mark
only the metrics guide, because it is the latest message. Or: a run goes red, you fix it and wait for green without
reopening the review, so the owner merges a commit they never saw.

## The build-stage message

Exactly, per issue in landing order: the issue number and the branch; the
PR's URL and base; a summary of the change in plain words; each deviation
from the plan with the reason the code forced it; the comment checks in
three lines, which are the last line of the step check at the reviewed
head, the number of parts read and the number of stops of each reader round
with how many each way closed, and the audit's findings applied and
declined, each line "not run" when its saved file is missing, followed by the
audit's "Needs a human" findings; the review's outcome, with the Inbox
message id the owner marked; the run ids and their state for the reviewed
head, and any superseded heads, or the local checks that ran when there is
no CI; the command `git diff <base>...<slug>`. After the issues, one line
naming the local services the build started and removed, or "none". Then one
line saying the merge waits for `/tk-merge`.

Then wait. Do not ask whether to merge; the owner invokes `/tk-merge` when
they are ready.
