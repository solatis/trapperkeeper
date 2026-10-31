# Report, then stop

Reply with exactly these sections, in this order. Every candidate, every
constraint and every code claim carries a path, for example
`doc/architecture.md` or `server/src/sync.rs:42-46`; a claim without a path
is not made. Do not summarize the documents; report only what the unit
must honor.

Each section is a markdown heading, `# Name`, on a line of its own with a
blank line before and after it. Sections carry no numbers. A numbered list
appears only where a section asks for one, and it starts at 1. The
Candidates section belongs to `/tk-start-next` only; when its step did not
run in this seed, it is omitted, not written as "none".

- `# Workflow` -- one line acknowledging the workflow and naming the
  branches you will use, not yet created.
- `# Position` -- at most five lines: the active milestone and its goal,
  what is in flight (open PRs, plans), how the unit relates to them.
- `# Candidates` (`/tk-start-next` only) -- a table, one row per
  candidate, in the order they were offered:
  `| candidate | source (path or #n) | tree check (landed / blocked / ready) | verdict |`
- `# Brief` -- prose for the owner to read, at most 200 words, no tables,
  no paths: the one section whose claims the sections below evidence
  instead of carrying paths themselves. Three paragraphs: what the unit is
  and what is true when its issues are merged; how it will be done, the
  approach and the shape of the change in plain words; how it fits the
  milestone's goals and what is in flight, why it comes now, and what it
  leaves for later. Written for a reader who has not opened the sources.
- `# Issues` -- one block per issue, in landing order:

  ```
  <n>. <Issue title>   (#<number>, or "new")
       slug: <slug>   base: master | <slug>   PR title: <type(scope): subject>
       <one line: what the issue delivers>
  ```

- `# Decisions` -- a numbered list, one entry per design choice from Shape
  each issue, one to three lines each: the issues it applies to, the
  choice, the alternative it beats, the reason, and `settled by <path>` (an
  `owner` source), `settled by owner, <date>` (the challenge round) or
  `proposal`. Write "none" if the unit leaves no choice to make.
- `# Constraints` -- a table, one row per decision, constraint or gotcha
  that applies:
  `| constraint | source (path or path:line) | provenance (owner / tree / agent) | effect on the unit |`
- `# Code involved` -- one line per file, crate or directory in the
  scoped set: path, then what it does and how the unit touches it.
- `# Open questions` -- a numbered list of everything the unit's issues or
  sources contradict or leave open, every stale entry or gap the survey
  found, every choice Shape each issue found was not yours to make, and
  every challenge the round did not reach or the owner deferred. Ask; do
  not resolve by assumption. Write "none" if there are none.
- `# Proposed commits` -- per issue, under its slug, numbered one-line
  commit subjects in the order you would land them: the outline the plan
  will refine, so the owner can redirect before the plans are written.
  Each list closes with its Verify step, which is not counted as a commit.

Correct: the brief says the server stamps the receive time on each event
before it appends to the log, so a sensor with a wrong clock cannot reorder
the log; it names trusting the sensor's timestamp as the alternative and
says why it lost; the decision entry repeats the choice with what settles
it.

Incorrect: the brief says "add event ingestion, 2 issues, 11 commits", and
the owner learns what ingestion does from the commit subjects.

Then wait. The owner opens the plan stage with `/tk-plan`; nothing else
does.
