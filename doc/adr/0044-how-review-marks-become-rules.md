# 0044. How review marks become rules

Date: 2026-11-14.

## Context

The owner reviews every plan and every pull request, and what he marks there
is the best evidence of where the rules fall short. Until now a mark led to a
fix at the marked place. The lesson behind it went into a private memory note
or nowhere, so the same kind of mark came back in later units.

The one path from a complaint to a rule was a request in the chat. A rule
change made that way answered a complaint about too much prose with limits
on every comment, and the opposite complaint followed within hours. A rule
change therefore needs two things: a regular moment at which it is
considered, and requirements that keep it from overshooting.

## Decision

After the last pull request of a unit has merged, the merge stage runs a
postmortem. It reads what the owner marked in the unit's reviews and sorts
each mark as a decision of the owner, as one place, or as a class of defect
that recurs. For each class it proposes one change to a rule, or one check,
and it puts the proposals to the owner in one message (owner, 2026-11-14).

A stage asks the owner for a rule change in the postmortem and at no other
moment. During a review the build fixes the marked places, and its next
guide names the rule sentence it had followed (owner, 2026-11-14).

The postmortem runs in a subagent of its own that has not seen the session
that built the unit. It returns the text of the message, and the merge stage
sends it.

Every change to a rule file meets the same requirements, whoever asks for
it. They are the section "Changing a rule" of
`.claude/skills/tk-workflow/SKILL.md`. In short: the change quotes the marks
behind it and touches rules for that kind of place alone. It needs marks
from two reviews, or the owner's words that the defect recurs. It says which
of four things failed, and a rule that was not applied gets a check and no
further text. It replays the new sentence against the marks and against the
owner's earlier complaints in the opposite direction. It prefers a rule
about the kind of a thing to a rule about its quantity.

The history that these requirements need lives in the decision records. A
rule of weight has a record, and a change that brings back one of its
rejected alternatives has to answer the reason given there
(owner, 2026-11-14).

An accepted proposal lands as a rules pull request inside the merge stage:
one small branch, one commit per proposal, its own guided review, and a
fast-forward after the owner's "Looks good" on a green head
(owner, 2026-11-14).

A lesson about how the owner wants the work done goes into a rule file. A
memory note may point at the rule, and it holds no lesson of its own.

## Rejected alternatives

A question to the owner during the review, at the moment he marks the
defect. A review message with an open question cannot be marked "Looks
good", and a class shows only when the reviews of all pull requests of the
unit are counted together.

A ledger file of rule changes with their causes. The repository keeps no
historical trail of decisions, and a second file beside the records would
drift from them.

An issue for the next unit that carries the accepted changes. A rule change
is often one sentence, and the next unit would be built under the old rule.

A change to the rules on the owner's request alone, without requirements.
Such a change can answer marks about one kind of place with a rule for
every place, and it can set a quantity where the complaint was about a kind.

The session that built the unit sorting its own marks. It reads the marks
the way it read the rules while it built, so it cannot tell an unclear rule
from a rule it skipped.

## Consequences

A unit whose reviews show no class sends the owner nothing, and its
postmortem is one line in the merge report.

A unit with a class costs the owner one message of at most four questions
and one small review.

A rule changes at the earliest after the unit in which its defect showed for
the second time, so the first unit after a new kind of mark is still built
under the old rule.

The search for earlier complaints in the opposite direction works on the
owner's wording, and it misses a complaint that he worded in a way the
search did not try.

The requirements hold for a change the owner asks for in the chat only when
the session has loaded the workflow skill. The root `AGENTS.md` points at
the section for that reason.
