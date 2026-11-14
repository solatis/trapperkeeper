# The plan document

## The plan is the issue's only memory

The build stage may run in another session, by an agent that has none of
this conversation: not the seed report, not the owner's answers, not what
was tried and dropped. The plan is the only thing that crosses that
boundary. It is never committed and the merge stage deletes it, so a reason
it carries is lost unless it lands in a comment, a document or an ADR. Most
of the plan's reasons should land. Some should not: an answer the owner gave
in the conversation, or an alternative no reader of the code would propose.
The plan marks those `plan only`, and they die with it on purpose.
Write it for that reader: an executor who writes every commit, every
comment and every document row from the plan alone, without guessing and
without asking, and who could defend each choice to the owner the way you
can now.

Step 2 of `/tk-plan` has loaded the doc-discipline skill and read its
`narrative.md` and `placement.md`. They give the comment shape and the
placement ladder the build stage is held to, so the plan's comment
specifications and homes name that shape from the start rather than
repaired after. The skill's vocabulary names the shapes; the plan is still
read by someone who has not loaded it, so every shape and reason is said in
plain words.

## Header

The plan starts with the header lines that `/tk-workflow` fixes under
Plans, then a blank line, then the sections:

```
Issue: #14
Branch: metrics-endpoint
Base: server-logging
PR title: feat(server): expose prometheus metrics
Thread: server observability
Status: draft
```

## Sections

These sections, in this order. A section with nothing to say says "none";
it is never dropped.

1. **Outcome.** What is true in the tree when the issue is merged, and what
   it leaves for later issues, each named.
2. **Rationale.** One row per decision the issue rests on:
   `| decision | why | rejected, and why | gained | given up | settled by |`.
   It comes before the Design because the owner reads the plan top to
   bottom, and a choice they have not seen cannot be ruled on. "Settled by"
   is a path, "owner, <date>", or `proposal`. An owner row needs the
   decision and the date; its "why" is the owner's reason when they gave
   one and "owner's call" otherwise. A proposal row is a choice the owner
   has not ruled on, so it carries the reason and the alternative that
   lost, or it is a question asked before the gate. Every new type, route,
   table, column or module the owner did not name is a row marked
   `proposal`, so no choice hides in a sketch. "Rejected, and why" names
   the alternative that lost and what sank it, so no later agent
   re-litigates it. "Gained" and "given up" are the trade: what the issue
   buys, and the cost it accepts (a limitation, a slower path, a rule the
   code must now keep). Put here everything the conversation established
   that the code will not show: a dependency's defect, an owner preference,
   a reversal of an earlier rule. A decision another plan of the unit also
   rests on is a row here too, worded the same.
3. **Verified facts.** What you checked against dependencies, crate docs,
   a running tool or the documents, each with its evidence (`path:line`, a
   commit, a command's output, a dated session), so the executor neither
   re-verifies nor trusts what was never verified.
4. **Design.** Every function, type and constant the issue adds or
   changes, in the file order they will have. Each entry is a sketch: one
   fenced block in the language of the code, holding the doc comment above
   the signature, the signature, and a body made of comments where the
   real comments will sit. The first body comment names the shape as a
   `style:` line from the vocabulary the example below fixes, followed by a
   `because:` line giving the reason for it in plain words a reader without
   the skill can follow ("one match over the event kinds"; "the batch is
   validated whole before anything is appended, and three things fail on
   the way"). Below it, one claim per comment bullet, each ending with its
   evidence in brackets (`path:line`, a test, a Rationale row). The doc
   comment holds one to three contract facts. A function that qualifies
   for a walkthrough under the root `AGENTS.md`, "Code comments", is
   sketched with the walkthrough in full. The walkthrough opens with what
   the function does and how, and then has one numbered item per step. An
   item says what its step does, and it gives the step's reason when the
   step has one. Below the walkthrough, the sketch puts one back-reference
   per item at the place where its step will begin. A back-reference is a
   one-line comment with the item's number and a few words from the item,
   and it carries no claim and no evidence. A note claim sits at the line
   or construct it explains. It gives a reason, such as why a guard, a
   threshold, a constant or an order is there, or it says what a dense line
   achieves, such as a closure, a pattern, a combinator chain or arithmetic
   on indexes. A rejected alternative appears at a line only when a reader
   of that line would propose it. A pseudo-code statement may anchor a step
   (`let rules = active_rules(&tags)?;`) and is illustrative: the code
   commit follows the signature and the claims, not the sketch's
   statements. A claim is one sentence, and two claims never
   share a bullet.

   The sketch is the shape and the claims, not the wording. The executor
   rewrites every comment at build time, with the code in view, in the
   shape the root `AGENTS.md`, "Code comments", defines. It keeps every
   bullet as a claim and drops the evidence brackets, which stay in the
   plan. The claims are the least the code carries. The build adds a note
   at every line that makes the reader stop and that the sketch did not
   foresee. A remark of the owner about comments rewrites the sketches it
   concerns: their style lines, walkthroughs and notes. The plan does not
   record the remark as a paragraph beside the sketches, because the build
   follows the sketches and not the paragraph. Where the issue changes
   existing code, the sketch names the comment blocks the change
   touches, and the build rewrites them whole from the new code. Document
   rows (an `AGENTS.md` bullet, a `doc/` paragraph) are sketched the same
   way: a fenced block of bullets under the heading they will join. Tests are
   designed the same way.

5. **Knowledge.** Per commit, where each why of sections 2 and 4 lands:
   the commented functions with the evidence for each reason of every claim;
   the `AGENTS.md` sections (What, Map, Invariants, Why, How to) and `doc/`
   passages the commit changes; every fact with no home yet, with the home
   it gets and the commit that gives it. When the issue creates a
   component directory, `doc/AGENTS.md`'s eviction rule applies in the same
   commit and is listed here: the component's `doc/architecture.md`
   section shrinks to one paragraph and a pointer, and its
   `doc/decisions.md` entries move into the new `AGENTS.md` under **Why**
   and are deleted from `doc/`. A Rationale row settled by "owner, <date>"
   lands in a Why entry marked the same way. Every Rationale row names its
   landing place here, which is one of five. A decision that shapes more
   than one component lands in an ADR in `doc/adr/`. One that shapes a
   component lands in its `AGENTS.md` under Why. A reason about a whole
   step of a function lands in that step's walkthrough item. A reason that
   explains one line lands as a note at that line. A reason the code will not
   raise again, such as an owner's answer in the conversation or an
   alternative no reader would propose, is marked `plan only` and dies with
   the plan.
6. **How the knowledge lands.** One line: the build stage is `/tk-build`,
   which writes every comment and document row from sections 4 and 5 in the
   same commit as the code and has `/doc-audit` reconcile the code against
   this plan before the push. Below it, only what is specific to this
   issue: a commit whose comments depend on a fact the executor must check
   first, a document row that must land before a code commit can cite it, a
   reason the executor must ask the owner for if the code turns out a
   certain way.
7. **Commits.** The numbered list, one subject each in the commit form of
   the workflow, in landing order: the build commits, then the comment
   checks the build stage makes after the last of them (the step check, the
   `/doc-read` rounds and the `/doc-audit` run, whose commits depend on
   their findings), then the Verify step, last. When the issue changes a
   file whose comments do not pass the step check, the list holds one
   `docs` commit that brings that file's comments into the shape of the
   root `AGENTS.md`, "Code comments", before the first commit that changes
   the file. The step check reads the whole of each file a branch changed,
   so the build cannot reach the push without that commit.
8. **Open questions and recommendations.** Every question the owner has
   not answered, with your recommendation, one line each. The gate's
   approval settles them.

## The sketch

Correct: each Design entry is a sketch in this form, with its style named
on the first body line:

```rust
/// <contract claim> [evidence]
/// <contract claim> [evidence]
pub fn <name>(<params>) -> <result> {
    // style: walkthrough
    // because: <reason for this style in plain words>
    //
    // <what the function does and how> [evidence].
    //
    // 1. <what step 1 does> [evidence].
    //    <why step 1 exists or comes at its place> [evidence].
    // 2. <what step 2 does> [evidence].
    // 3. <what step 3 does> [evidence].
    //    <why step 3 exists or comes at its place> [evidence].

    // 1. <a few words from step 1>
    <pseudo-code anchor, illustrative>

    // 2. <a few words from step 2>
    <pseudo-code anchor, illustrative>
    // at <line or construct>: <reason or what the line achieves> [evidence]
    <pseudo-code anchor, illustrative>

    // 3. <a few words from step 3>
    <pseudo-code anchor, illustrative>
}
```

```rust
/// <contract claim> [evidence]
fn <name>(<params>) -> <result> {
    // style: notes
    // because: <reason, e.g. one match over the event kinds>
    //
    // at <line or construct>: <reason or what the line achieves> [evidence]
}
```

Code in another language uses its own doc-comment syntax in the same form.
The style line names the level of the root `AGENTS.md`, "Code comments":
`bare`, `notes` or `walkthrough`. Any of them may add
`+ Note: on <ownership | allocation | I/O>`. The planner tests the levels in
the order that section gives. A function at the level `bare` has a sketch
with the doc comment, the signature and the two lines `style:` and
`because:`. Its `because:` line says why none of its lines makes the reader
stop, such as "it maps the row's fields one to one". A claim of a
walkthrough ends with a period after its evidence bracket, because the step
check reads a number as a new item only after a line that ends a sentence.
A back-reference in a sketch is one line. The line below it is code, an
empty line or a note whose text starts in the column of the
back-reference's number, so that `check_steps.py --markdown` reads the
sketch as it will read the code. A rejected alternative at a line cites its
Rationale row. A question the plan cannot answer is asked before the gate,
never written as a claim.

Incorrect: the plan pastes the comments as finished Rust and tells the
build to type them in, so the wording is frozen before the function exists;
or it describes the comments in prose ("a doc comment and a numbered
walkthrough; claims: ...; ..."), so a reviewer rebuilds the function in their
head to see what is covered; or the style line says "commented (rule 3)" and
nothing else; or the Rationale says what was chosen and not what lost, so
the next issue tries the loser again. Or the walkthrough items are one word
each and the explanations sit at the numbers in the body, so the reader
meets the approach in fragments. Or a number appears twice in the body, or
a back-reference is a bare number with no words, so the build copies a
numbering that the step check rejects. Or a walkthrough item states a fact
about one line, which belongs in a note at that line. Or a parser of thirty
functions carries a handful of note claims, so the build lands those and
leaves the closures and patterns of the helpers bare. Or every item names a
rejected alternative, so the code reads like the planning session.

## The plan ends with a verified head

Local checks cover one machine; the pipeline covers what CI runs. So the
Commits section closes with one final step that is not a commit, written
out in the plan itself:

> N. Verify: push `<slug>`, open its PR against `<base>`, watch its head's
> Actions runs. Green: report the run ids. Red: fix with further small
> commits on this branch, push, watch again. No `.github/workflows/` in the
> tree: run the local build, lint, tests and `npx prettier --check` on the
> touched Markdown, and say that no CI ran.

A plan that changes only documents still gets the step. How the runs are
watched, and what a red run means, is `/tk-build`'s to say.

Incorrect: the plan lists only commits, so nothing at build time says to
push and watch the runs.
