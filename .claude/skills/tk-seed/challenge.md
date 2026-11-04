# Shape each issue

The report explains the unit to the owner before it lists anything, so
settle its shape before writing. From what you read, decide for each issue:

1. The outcome: what is true in the tree when the issue is merged, and
   what it leaves for later.
2. The approach: the shape of the change, where it lives, what it reuses,
   what it introduces.
3. Its base: `master`, or the issue whose code it needs, which makes it a
   stacked branch on that issue's branch. Stack only when the code needs
   the other issue's commits, or when the owner would never merge one
   without the other; an independent issue can merge alone.
4. Each design choice it requires: the option you take, the alternative it
   beats, the reason, and what settles it. A choice an `owner` source
   settles carries that path. A choice only `agent` sources settle, or that
   the sources leave open, goes to the owner in the challenge round below:
   what the owner answers is settled by the owner, and what the owner
   leaves to you is a proposal: you still pick, you say the pick is yours,
   and the plan gate's approval is what settles it. A choice that
   contradicts a settled decision, or that would make an issue materially
   different work, is not yours to make; it goes under Open questions.

A choice two issues share (one log format for logging and metrics) is
settled once, for the unit, and both issues carry it.

# Challenge round

The plans are written after this report, and they carry the approach the
owner wants, not the approach the documents happen to record. So before
the report, put the choices to the owner through the question tool, while
the context is loaded and nothing is written.

What gets a question. A choice an issue's code rests on that is:

1. `agent` by provenance, including every detail of
   `doc/architecture.md` the unit would be the first to implement;
2. drift between sources, or between a source and the tree;
3. expensive to reverse once code exists: a wire shape, a public
   signature, a config key, a storage layout, a dependency, a crate
   boundary;
4. one where a simpler alternative exists and no source records why it
   lost.

What gets no question: a choice marked `owner`; a choice the tree settles;
a choice that belongs to a later issue; a choice between wordings. A
question whose answer would not change a commit of this unit is not asked.

How to ask. One `:::question` block per choice, at most four per message,
at most two messages for the whole unit, in the unit's Inbox thread as
`/tk-workflow`, The Inbox, says. End your turn after each message; the
report waits for the answers. What remains goes under Open questions in the
report, and the owner answers it before opening the plan stage.

The owner reads the question cold: the context you loaded is not in front
of them, and a name from the code means nothing until the question says
what it is. So the question text carries the context, in this order,
before it asks anything: what the thing is and what it does today, in
plain words ("the sensor asks the server for its rules; the server answers
the rules that match the sensor's tags"); what the issue changes about it
and why that raises a choice ("this issue adds the first version tag, and
every later sync compares against it, so its form is fixed now"); then the
question. An identifier appears only after the sentence that explains it,
in parentheses, so the owner can find it in the code. The question runs
three to six sentences; a one-line question is a sign the context was left
out.

The choices follow the same rule. Each choice is one line,
`- [ ] label - what it does, what it costs and what it keeps open`. The
label says the choice in plain words, not in identifiers ("a hash of the active rule set",
not "sha256(rules)"), and the rest names an identifier only after saying
what it is. The choices are: the approach the sources record, named by its
path; the simplest alternative; a third where a real one exists. Keep them
in that order. After them, a `Recommended: <label>` line names the choice
you would take, and a `Why:` line says why in one clause. The owner can
always answer "Other" with text. An answer is the decision; it is not
argued with.

Where the answers go. Each answer is the owner's decision as of today's
date: in the report's Decisions entry as `settled by owner, <date>`, and in
the plan's Rationale row under "settled by" in the same words, so the build
stage can land it in the component's Why marked the same way. A choice the
owner left to you is `proposal` and keeps your recommendation. An answer
that gives an issue a new direction reshapes it before the report is
written: run Shape each issue again for what the answer touches.

Correct: `doc/architecture.md` says sync is "conditional on a version tag
derived from the active rule set", no document says how the tag is
derived, and the unit adds the first sync endpoint. The question says: a
sensor asks for its rules on every sync; the server answers nothing when
the rules did not change; this issue adds the tag that decides that, so
its form is fixed for every SDK now; the architecture document says only
that it is derived from the rule set; should it be a hash of the active
rule set or a counter bumped on every change? The owner picks the hash;
the Decisions entry says `settled by owner, <date>`, and the plan's
Rationale repeats it.

Incorrect: the hash is chosen because it sounded right, and the owner
learns the tag's form from the diff. Or: the question is "Should
`sync_tag()` return `u64` or `[u8; 32]`?", so the owner has to open the
code to learn what the tag is. Or: eight questions go out in one call,
three of them about an issue the owner did not choose. Or: the hash is
listed first with "(Recommended)" in its label, so the order says which
one you want before the owner has read the alternatives.
