# 0043. How code is commented

Date: 2026-11-14.

## Context

Agents write the comments in this repository, and the owner reads them in a
diff. Two complaints kept alternating. After a rule asked for more
explanation, agents wrote long prose above simple functions and narrated the
code. After a rule asked for less, agents left dense code bare and cited the
rule's limits to do so. The parser of the rule language needs explanation at
nearly every helper, and a handler that maps fields needs none. A rule about
the amount of comments cannot serve both.

The build stage also ran its own comment checks in the context that wrote the
code and reported them as clean. Nothing showed the owner that no check had
run.

## Decision

A function that needs it starts its body with a walkthrough. The walkthrough
opens with what the function does and how. Numbered steps follow. Each step is
one or more full sentences that say what the step does and, where there is
one, why it exists or comes at its place. Each step has one back-reference in
the code, a one-line comment with the step's number and a few words at the
first line of the step. The back-references appear in ascending order, and a
step may begin inside the block of the step before it. Any other comment in a
body is a note, which carries no number and sits directly above the line it
explains. A function that forwards, passes settings through or maps fields one
to one has no comment in its body (owner, 2026-11-14).

A comment is written for a senior engineer who reads a diff cold. That reader
knows the language, its standard library and the common algorithms by name.
The reader has no type hints, has not read the plan, the decision records or
the rest of the module, and does not know the terms the code defines (owner,
2026-11-14).

A fact sits where its scope is. A fact about a whole step, about the order of
the steps or about the approach goes in the walkthrough. A fact about one line
goes in a note at that line. Each fact is stated once. A comment may name one
example input where the effect is hard to see, and a byte offset appears only
where the code computes a range. Numbers belong to walkthroughs, and a list
inside a note uses bullets (owner, 2026-11-14).

A plan sketches a function that needs a walkthrough with the walkthrough in
full and its back-references. The claims of a sketch are the least the code
carries, and a remark of the owner about comments rewrites the sketches it
concerns (owner, 2026-11-14).

A script in the doc-discipline skill, `check_steps.py`, checks the numbering
of walkthroughs and back-references in any language, in code and in the fenced
code blocks of Markdown. It also flags a comment that names a path under
`doc/adr/` or carries a who-decided mark. The plan stage runs it on each plan.
The build and merge stages run it on the whole of every file a branch changed
(owner, 2026-11-14).

The session that owns a change writes its comments and documents, from
evidence it can name, and it asks the owner where it has none. After its last
code commit, the build stage runs the step check. Then `/doc-read` gives each
part of the change to a reader that has seen neither the plan nor the
conversation, and the reader lists where it had to stop. The session closes
each stop with a rename, a comment from evidence, a question to the owner, or
a decline with its reason. A second round of `/doc-read` reads what the
session changed to close the stops. Then `/doc-audit` checks the change
against the policy and reconciles it against the plan, and the session
applies or declines each finding. The answers of the two skills are saved under `.plans/`, and the
guided review shows the stops, the declines and the findings from those files
(owner, 2026-11-14).

The cold read takes a change in parts of about 300 changed lines, and one
reader reads each part. The script `change_parts.py` in the doc-discipline
skill lists the parts.

The doc-discipline skill is a guide that an agent loads once. It carries no
policy of this repository, and the root `AGENTS.md` holds that policy. The
step check is a script that the stages run as a command. `/doc-read` and
`/doc-audit` each run in a subagent of their own, which gets its whole task
from the text of the skill and has no tool that writes (owner, 2026-11-14).

## Rejected alternatives

A terse numbered list at the top of the body, with the reasons at the numbered
steps. The reader meets the approach in fragments, and the list cannot say why
the steps come in their order.

Limits on quantity, such as a share of comment lines or "two lines at most".
Agents cite such a limit to leave dense code bare, and a rule about the amount
of comments does not answer a complaint about their kind.

A comment only where a line raises "why?". Most places where a reader stops
raise "what does this do?": a closure, a destructured tuple, a chain of
combinators or arithmetic on indexes.

The step check as a crate run by `cargo test`. The check has to work for every
language and move with the skill to other repositories.

A cold agent that writes the missing comments. It does not know the reasons,
so it fills the gaps with an example at nearly every line, byte offsets and
paraphrases of the code, and some of what it writes is false.

The author judging its own gaps. The author already understands the code, so
it cannot see where a cold reader stops, and its pass leaves no trace the
owner can check.

One reader over a whole change. On a file of 1070 lines, one reader finds 9
to 11 of the 16 places the owner marked and reports almost nothing in its last
150 lines. Three readers on a third each find 15.

One doc-discipline skill invoked with mode words for each check. The harness
loads a skill as text, and a second invocation of a loaded skill adds only a
note. An agent then carries out a mode in its head and leaves no output.

## Consequences

Existing code keeps the old shape until a follow-up brings it over. A branch
that changes a file in the old shape brings that whole file over, because the
step check covers every file a branch changed.

The build stage starts one subagent per part in each reader round and one for
the audit, and its questions to the owner go out as one Inbox message per
issue.

The step check cannot see a back-reference at the wrong line, a function that
needs a walkthrough and has none, or a comment that is false. The reader and
the audit are there for those.

`/doc-read` and `/doc-audit` rest on one behavior of the harness that its
documentation does not confirm: a command line inside a skill that runs in its
own subagent. The two skills are tried once before this change merges.
