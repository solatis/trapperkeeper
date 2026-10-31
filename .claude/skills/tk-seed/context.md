# Load context

The `AGENTS.md` hierarchy is the map. The root file's "Where to look" table
says which document answers which question, and the `AGENTS.md` of each
component directory says what that component is and how to change it.
`doc/AGENTS.md` gives the rules of the documentation folder, including its
eviction rule. Follow that map rather than a list kept here, so this skill
stays correct as the repository changes.

First, scope. From the unit's issues and the project structure as the
documentation describes it, write down the set the unit touches:
directories, crates or packages, endpoints, config blocks, tests,
documents. This set decides what you read below; extend it when reading
reveals a dependency.

Then read, in this order, skipping what the caller's survey already
covered:

1. The issues themselves: each issue's title and body (`gh issue view <n>`)
   and the active milestone's description (`gh api
repos/{owner}/{repo}/milestones`), for the goals the unit serves.
2. The documents the root "Where to look" table routes the unit's subject
   to, whole: `doc/principles.md` always, `doc/overview.md` and
   `doc/architecture.md` for the components in the scoped set, and the
   `doc/decisions.md` entries those components carry.
3. The `AGENTS.md` of every directory in the scoped set, and of each
   parent directory on the way there.
4. The code in the scoped set and what it depends on, through the
   `codebase-analysis` skill, invoked with the Skill tool once the scope is
   written down, when the scoped set holds code. The skill's own description
   says to run it before any reading; here the documents come first because
   they fix the vocabulary and the scoped set the skill is given. Its goals
   are the unit in
   one sentence and the scoped set: the crates or packages, their tests,
   the call sites, and for dependencies the parts the unit will call. Its
   summary is the map of the code, not evidence: a claim from it enters
   the report only after you have read it back in the tree at a
   `path:line`, and until then it is `agent` by provenance like any other
   written account. Read the functions the unit will touch whole, top to
   bottom, whatever the summary says about them.
5. The history of the scoped paths: `git log --oneline -- <paths>`, and
   the diffs of the commits that introduced or last changed the functions
   the unit will touch, so the unit continues the existing direction
   instead of restarting it.
6. Plans in flight: every `.plans/*-plan.md` whose issue is still open, for
   work the unit would collide with or build on.
7. Memory: any recalled note about this repository that bears on the unit,
   verified against the tree before relying on it.

When sources disagree: the tree wins over any document for what exists
today, and an `owner` claim wins over an `agent` claim for what should
exist. Between two `agent` claims, or between an `agent` claim and the
unit description, there is no precedence. Every disagreement is drift,
and drift is a question in the challenge round, with both readings and
their paths; it is not resolved by assumption, nor by picking the newer
document.

Look actively for: settled decisions the unit must honor, entries in
`doc/decisions.md` whose code home the unit creates (they move with it),
sections of `doc/architecture.md` the eviction rule shrinks once the unit
creates a component directory, known gaps in the dependencies the unit
will call, what the existing tests and conformance fixtures already pin,
and anything in the unit description that contradicts a settled decision.
