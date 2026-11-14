---
name: tk-merge
description: Open the merge stage -- per PR in dependency order, fast-forward master onto the reviewed green head, push it, retarget stacked children, delete the branch, close the issue, delete its plan, close the milestone it completes, and after the last PR of a unit run its postmortem. Invoked by the owner after reviewing the diffs; the invocation is the owner's yes on the code.
disable-model-invocation: true
argument-hint: "[#n ... , issues or PRs to merge; empty means every PR of this session's unit]"
---

# /tk-merge -- the merge stage

Run `/tk-workflow` first. An argument, when the owner gave one, is:

<issues>
$ARGUMENTS
</issues>

The PRs to merge are those of the issues the argument names (an issue
number or a PR number), else those the build-stage message of this session
names. With neither, stop and ask.

## Entry check

Current branch: !`git branch --show-current`

Working tree (empty means clean):

!`git status --short`

Master vs origin, commits ahead / behind (0 0 means in sync):

!`git fetch -q origin 2>/dev/null; git rev-list --left-right --count master...origin/master`

Plans on this machine:

!`grep -H -E -r --include='*-plan.md' '^(Issue|Branch|Base|Thread|Status):' .plans 2>/dev/null || echo none`

Open pull requests, with the head SHA and the state of each check on it:

!`gh pr list --state open --json number,headRefName,baseRefName,headRefOid,statusCheckRollup --jq '.[] | "#\(.number) \(.headRefName) -> \(.baseRefName) \(.headRefOid[0:12]) \([.statusCheckRollup[]? | "\(.name)=\(.conclusion // .status)"] | join(" "))"' 2>&1 || true`

CI workflows in the tree (empty means no CI):

!`ls .github/workflows/ 2>/dev/null || true`

Step check of each open pull request's head branch against its base branch,
the last line per branch (0 findings means clean):

!`if prs=$(gh pr list --state open --json headRefName,baseRefName --jq '.[] | "\(.baseRefName) \(.headRefName)"' 2>&1); then printf '%s\n' "$prs" | while read -r b h; do [ -n "$h" ] && echo "$h: $(python3 .claude/skills/doc-discipline/scripts/check_steps.py --deny 'doc/adr/' --deny '\(owner,' --git-range "origin/$b...origin/$h" --markdown-too 2>&1 | tail -n 1)"; done; else echo "$prs"; fi || true`

The tree is clean, master is in sync with origin, and for each PR to merge:
it is open; its head SHA on GitHub is the local branch's head (`git
rev-parse <slug>`); and with CI in the tree, every check on that head is
`SUCCESS`, `SKIPPED` or `NEUTRAL`. A step-check finding on a pull
request's branch, or a step check that did not run for it, is a stop for
that pull request and for every pull request stacked on it. The checks are
read from the list above or from `gh-watch.sh <sha>` (exit 0), never from
the conversation: a build-stage message that says green is a claim, and
GitHub is the evidence.
Anything else (a dirty tree, a head that differs, a check `IN_PROGRESS`,
`QUEUED` or `FAILURE`, a PR whose base is a branch not being merged in this
run) is a stop for that PR and for every PR stacked on it; say which, and
the owner decides. The others proceed.

## Step by step

For each PR, in landing order (a PR after the PR its base branch belongs
to):

1. Make it a fast-forward. When master moved since the branch was cut (an
   independent PR merged before it), `git merge-base --is-ancestor master
<slug>` fails: rebase it onto master (`git rebase master <slug>`), then
   `git range-diff master <old head> <slug>` must show every commit
   unchanged in content. A conflict, or a commit whose content changed, is
   a stop: the reviewed code is no longer the code that would merge.
   Otherwise `git push --force-with-lease`, and with CI wait for the new
   head with `gh-watch.sh` (exit 0; exit 3 means rerun with the same SHA)
   before going on. A stacked PR whose
   parent was fast-forwarded in this run is already a fast-forward.
2. Fast-forward and publish, with the github skill's "Fast-forward merge":

   ```
   git checkout master
   git merge --ff-only <slug>
   git push origin master
   ```

   No merge commit, ever. A rejected push (origin moved, a ruleset that
   wants checks this head lacks) is a stop: report it; never force.

3. Verify: `gh pr view <n> --json state -q .state` is `MERGED`. When it is
   not, report it and leave the PR, the branch and the issue as they are.
4. Retarget every open PR whose base is this branch, before the branch
   goes: `gh pr edit <child> --base master`.
5. Delete the branch locally and on origin: `git branch -d <slug> && git
push origin --delete <slug>`.
6. Close the issue: `gh issue close <issue> -c "Landed in #<n>"`.
7. Delete the plan and the files of the build's comment checks:

   ```
   rm -f .plans/<slug>-plan.md .plans/<slug>-read-1.md \
     .plans/<slug>-read-2.md .plans/<slug>-audit.md
   ```

   Every reason the plan carried is in the code or the documents. The build
   stage's `/doc-audit` run made sure of that.

8. Close the milestone this PR completed. Read the issue's milestone with
   `gh issue view <issue> --json milestone -q .milestone.number`, and skip
   this step when it has none. When `gh api
repos/{owner}/{repo}/milestones/<m> -q .open_issues` is 0, read the
   milestone's description and check each line of its "done when" list
   against master as it is now. When every line is met, close the
   milestone as `/tk-workflow`, Units and issues, says. When a line is
   unmet, leave the milestone open, because `/tk-start-next` cuts the next
   issue from that line.

## The postmortem

A unit is complete when no plan in `.plans/` names its thread any more.
After the last PR of this run, do this for each thread whose unit is
complete:

1. Run `/tk-postmortem <thread name>`, with the thread name and no other
   word. The skill runs in its own subagent, which has not seen this
   session. It returns the text of one message, or one line that starts with
   `no class`.
2. On `no class`, add that line to the closing lines below and send
   nothing.
3. Otherwise send the text as it came, with `send_message` in the unit's
   thread, and end your turn until the owner answers. When a review waits in
   the thread, send it after the owner answers that review.
4. Land what the owner accepted as a rules PR. Cut the branch
   `rules-<thread slug>` from master. Write one commit per accepted
   proposal, each with the change to the decision record that `/tk-workflow`,
   Changing a rule, asks for. Push, open the PR with an empty body, and send
   its guided review into the thread as `/tk-build`, The runs and the
   reviews, says. When the owner marks that guide "Looks good" and the head
   is green, fast-forward the PR by steps 1 to 5 above. A rules PR has no
   issue, no plan and no postmortem.

The owner's answer to a proposal is his yes on its text. It is not his yes
on the commit, which is why the rules PR gets its own review.

Close with one line per PR: `<slug>: #<pr> merged, <n> commits, <first
sha>..<last sha>, issue #<issue> closed`, plus "rebased first" when step 1
rebased, ", milestone <title> closed" or ", milestone <title> open: <unmet
line>" when step 8 read a milestone, and one line per PR that stopped, with
the reason. Add one line per postmortem: its `no class` line, or the number
of proposals sent and the rules PR that landed the accepted ones.

Correct: logging (#20) and metrics (#21, stacked on logging) are green and
approved. You fast-forward master to logging's head, push, see #20 MERGED,
retarget #21 to master, delete `server-logging`, close #12, delete its
plan; metrics is now a fast-forward of master, so you merge it the same
way. #21 closed the last open issue of M1, and master meets every line of
its list, so you close M1.

Incorrect: you delete `server-logging` before retargeting #21, and GitHub
closes #21. Or: the build-stage message said green, the list shows a check
`IN_PROGRESS`, and the merge runs on the message. Or: the push is rejected
and you retry with `--force`. Or: the seed report said the unit
finishes M1, and you close M1 on that claim without checking the list
against master. Or: you tell `/tk-postmortem` which marks you think matter,
so the postmortem sorts the marks the way the session that built the code
sees them. Or: the owner picks a proposal, and you commit the change to
master without a rules PR and its review.
