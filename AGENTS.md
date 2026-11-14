# TrapperKeeper

Data-quality rule engine for data pipelines. Pipeline code embeds a sensor that
evaluates centrally managed rules against records in-process and reports matches
to a central server. Rules change without redeploying pipelines, the server
never needs to know the shape of the data, and a broken TrapperKeeper never
breaks the pipeline.

## Where to look

| Question                                      | Read                    |
| --------------------------------------------- | ----------------------- |
| What is the product, for whom, vocabulary     | `doc/overview.md`       |
| Which components exist, how they talk         | `doc/architecture.md`   |
| What trade-offs do we always make the same    | `doc/principles.md`     |
| What is open, not yet decided                 | `doc/decisions.md`      |
| Why we decided something, what lost           | `doc/adr/`              |
| How does component X work, how do I change it | `<component>/AGENTS.md` |

## Layout

One top-level directory per component. The server side is a Cargo workspace
whose members are listed one by one in the root `Cargo.toml`. Only directories
that have code exist; the others are named here so the layout is fixed before
they arrive. A component directory has its own `AGENTS.md`, and inside
`server/` each module directory gets one as it appears.

| Directory      | What                                               | Exists                       |
| -------------- | -------------------------------------------------- | ---------------------------- |
| `server/`      | the server binary: APIs, store, `serve`, `migrate` | yes, see `server/AGENTS.md`  |
| `engine/`      | the rule engine crate, embeddable by pipelines     | yes, see `engine/AGENTS.md`  |
| `sdk/<lang>/`  | one sensor SDK per language                        | Rust with M7, Python with M9 |
| `web/`         | the browser UI                                     | with M8                      |
| `conformance/` | the language-neutral evaluation fixtures           | with M4                      |

## How to

- Toolchain: rustup with the `stable` channel. `rust-toolchain.toml` pins the
  channel and the clippy and rustfmt components.
- Format: `cargo fmt --all --check`.
- Lint: `cargo clippy --workspace --all-targets`. The workspace lints table in
  `Cargo.toml` denies clippy `all`, clippy `pedantic` and rustc warnings, so a
  lint failure is a build failure locally as in CI.
- Test: `cargo test --workspace`.
- CI (`.github/workflows/ci.yml`) runs those four on every push, on five
  native runners: Linux amd64 and aarch64, Windows amd64 and aarch64, macOS
  aarch64, always on the current stable. Its five job names are the required
  status checks of the `master` ruleset, which the owner keeps by hand. Each
  runner except Windows aarch64 also starts Postgres and MariaDB, so the tests
  run against all three backends; why is `doc/adr/0021`.
- The reasons behind the layout, the toolchain policy and the lint level are
  records `0010`, `0009` and `0011` in `doc/adr/`.

## Workflow

The owner is the repository's maintainer, the only person who opens a stage
or answers a question. "The question tool" is a `:::question` block sent
to the Plannotator Inbox in the unit's thread. After sending it, you end
your turn until the reply arrives. `.claude/skills/tk-workflow/SKILL.md`,
"The Inbox", says how.

Work moves issue by issue through four stages, each opened by the owner:
`/tk-start` or `/tk-start-next` (seed), `/tk-plan`, `/tk-build`,
`/tk-merge`. A stage opens only when the owner invokes its skill; a chat
message opens none. The rules all stages share (branches, commits, issues,
the uncommitted plans in `.plans/`, the fast-forward onto `master`) are in
`.claude/skills/tk-workflow/SKILL.md`.

## Documentation strategy

Knowledge lives next to the code that implements it. `doc/` is the exception,
not the default, and is meant to shrink as code appears.

1. Every directory that is a unit of understanding has an `AGENTS.md`. A
   directory gets one when a newcomer would otherwise need to read several
   files to know what it is for.
2. A component `AGENTS.md` contains, in this order and only when non-empty:
   - **What**: one paragraph, responsibility and boundaries.
   - **Map**: table of files and subdirectories, what each is, when to read it.
   - **Invariants**: what must stay true and is not obvious from the code.
   - **Why**: decisions that shaped this component, a few lines each, citing
     the record in `doc/adr/` where one exists.
   - **How to**: build, test, and the common change recipes.
3. Hierarchy: a parent `AGENTS.md` indexes its children and states what is
   common to them; it never repeats their content. Read root-to-leaf.
4. Detail that explains one file or function is a code comment, not an
   `AGENTS.md` entry.
5. Executable beats prose: wire contracts are documented by their schema,
   evaluation semantics by the conformance fixtures.
6. Docs change in the same commit as the code they describe. A change that
   makes an `AGENTS.md` wrong is incomplete.
7. Component directories are created together with their first code, not
   before. Until then their knowledge sits in `doc/`.
8. Things get deleted: when knowledge moves next to code, it is removed from
   `doc/`. History is git's job.

The rules specific to `doc/` are in `doc/AGENTS.md`.

## Code comments

A comment says what the code cannot say for itself, and it is written for the
reader. The reader is a senior engineer who reads a diff cold. The reader
knows the language, its standard library and the common algorithms by name.
The reader has no type hints, has not read the plan, the decision records or
the rest of the module, and does not know the terms this code defines. A
function can carry four kinds of comment: the doc comment above it, the
walkthrough at the top of its body, one back-reference per step of the
walkthrough, and notes at single lines.

1. The doc comment above a function is the contract for callers, in one to
   three sentences: what it does, what it returns and how it fails. It holds
   no implementation detail.
2. Each function gets one of three levels. A function that forwards, passes
   settings through or maps fields one to one is bare, whatever else is true
   of it. A constant in it still says where it came from. Test the other
   functions against the levels in this order, and the first that applies
   wins.
   - **Walkthrough**: the reader cannot see the function's plan at any single
     line. A function that runs a parse or an algorithm in several phases, a
     function that joins values from several other functions in a way no
     single line shows, and a function whose steps must run in an order that
     needs a reason all qualify. A helper that one of these functions calls
     qualifies only on its own merits. A chain of independent cases qualifies
     when the place of some case in the chain needs a reason. A standard
     algorithm qualifies unless its name already says which variant it is.
     Length and branch count do not qualify a function.
   - **Notes**: the reader would stop at one or more lines of the function.
     Each such line gets a note, and the body gets no walkthrough.
   - **Bare**: any other function. Its body has no comment, except that a
     constant says where it came from.
3. The walkthrough is the first comment in the body. It opens with a
   sentence or a short paragraph on what the function does and how. Numbered
   items follow, one per step, counting up from 1. Each item says in full
   sentences what its step does and, where there is one, why the step exists
   or why it comes at its place. Two steps that share one loop are one step.
   The walkthrough says what each step achieves and leaves the statements of
   the step to the code.
4. Two kinds of walkthrough have no numbered items and no back-references.
   The walkthrough of a standard algorithm names the algorithm and its
   variant and states its invariant. A body that tests a chain of independent
   cases, where the first case that applies returns, has no sequence of
   steps. Its walkthrough names the cases whose place in the chain matters,
   and the note at each of those cases says why.
5. A fact sits where its subject is. A fact about a whole step, about the
   order of the steps or about the approach goes in the walkthrough. A fact
   about one line goes at that line as a note. Each fact is stated once.
6. Each numbered item has one back-reference in the body. A back-reference is
   a one-line comment at the first line of its step, with the item's number
   and a few words from the item, such as `// 2. run the grammar`. Every
   number of the walkthrough appears once as a back-reference, in ascending
   order, and every back-reference has its number in the walkthrough. A
   back-reference states no fact of its own, and a bare number such as
   `// 3.` is not a back-reference. A step may begin inside the block of the
   step before it. Code before step 1, such as an early return, carries no
   number. A note about the first line of a step sits directly below the
   back-reference, as a comment line of its own that is not indented under
   the back-reference's words.
7. Every other comment in a body is a note. A note carries no number and sits
   directly above the line it explains. A line gets a note when the reader
   would stop there to work out what the line does or to ask why it is
   there. Readers often stop at a closure, a pattern that takes apart a value
   another function built, a chain of combinators, arithmetic on indexes, a
   guard, a chain of calls on an optional value, and a match arm whose place
   in the match matters. Such a construct gets a note when the reader would
   stop at it. The writer already knows what each line does and cannot see
   where a reader stops. It writes a note at each such construct whose
   effect neither a name nor the line itself states, and `/doc-read` finds
   the lines that the writer missed. The note says what the line achieves in
   the terms of the problem, or why the line is there. A note that repeats
   its line in other words is deleted, and so is a note that explains what a
   language feature does in general.
8. A comment names one example input only where the effect is hard to see
   without one, as at a grammar rule, an alternative of a choice or a match
   arm whose case is not obvious. Most notes carry no example. A byte offset
   appears only where the code computes a range.
9. Numbers appear only in a walkthrough and its back-references. A list
   inside a note uses bullets.
10. Each kind of comment has a normal size. A walkthrough item is one to
    three sentences, a note is one to three lines, and a back-reference is
    one line. A walkthrough item grows past its size only for a worked
    example that its steps use. A note that holds several facts is split, and
    each fact moves to the line or the step it concerns. No ratio of comment
    lines to code lines applies.
11. Tests follow these rules like any other code. A test that moves the
    system through several states before it checks the result gets a
    walkthrough of its scenario. A check whose expected value is not obvious
    gets a note that says why the value is right.
12. Constants say where they came from, and impossible branches say why they
    are impossible. A construct that looks wrong says which alternative was
    rejected, but only where a reader of that line would propose the
    alternative. Other rejected alternatives belong in `doc/adr/` or nowhere.
13. A comment describes the code as it is. When code changes, rewrite every
    comment block the change touches from the new code, as if the code had
    always worked this way. The walkthrough is one block, so a change to any
    step rewrites the whole walkthrough and renumbers the back-references.
    Write the new block before rereading the old one, then carry over only
    the facts the code cannot show. A comment never mentions an earlier
    version, a plan, a review, an issue, a unit of work or the conversation
    that shaped it.
14. Only state what is known. When the owner is in the session, an unknown
    reason, or a function you cannot place under the rules above, is a
    question to the owner through the question tool, asked before the commit
    that needs it. Never invent a reason.
15. After writing a function's comments, read them back top to bottom as
    separate claims and check each against the code below it. Check each
    walkthrough item against the code from its back-reference to the next
    one. Fix or delete any claim the code does not bear out.
16. A comment cites a decision record by its number, as `ADR-0020`, and not
    by its path. A comment does not say who decided something or when, so it
    holds no mark such as `(owner, 2026-11-10)`. The record in `doc/adr/`
    says how the decision was made, and it may carry that trace.

This is the primary path: new code is written this way. The doc-discipline
skill in `.claude/skills/doc-discipline/` is the guide for applying these
rules and holds the worked examples. An agent loads it once, before it writes
code, a plan or a document, and follows its `checks.md` after a change is
complete. Two checks run in a subagent of their own and change no file.
`/doc-read <base>...HEAD <part>` takes the range of a change and the number
of one of its parts, and returns each line of that part where a reader who
has not seen the change stops. `/doc-audit <scope>` takes a range or paths
and returns the changes these rules call for as rows of a report.
`/doc-audit <scope> plan: <plan>` also reconciles the code against the plan.
The session that owns a change writes its comments and documents from
evidence it can name, and it asks the owner where it has none. The step check
enforces the numbering of walkthroughs and back-references, and it reports
the two forms that the last rule forbids:

```
python3 .claude/skills/doc-discipline/scripts/check_steps.py \
  --deny 'doc/adr/' --deny '\(owner,' <path>...
```

It reads the whole of each file it is given, prints one line per finding and
ends with a line that counts them. It does not read block comments. It does
not check whether a back-reference sits at the right line, whether a function
that needs a walkthrough has one, or whether a comment is true. `/doc-read`,
`/doc-audit` and the read-back cover those.

## Prose

Comments and documents read like a colleague explaining the code, in plain
sentences. The rules:

1. Every sentence has a subject and a verb and says who does what. Write
   "the fixture compares the batch with the table it wrote", not "the
   comparison is the fixture's".
2. One idea per sentence. A colon introduces one example or one list of
   nouns, never a chain of clauses, and a semicolon joins nothing.
3. No ellipsis: a sentence does not drop its verb, and a possessive does not
   stand in for a clause ("the decoders' decode_test.go").
4. No rhetoric: no "X, not Y" contrasts except to record a rejected
   alternative, no aphorisms, no mirrored clauses, no "the one", "and
   nothing else", "exactly" or "never" added for emphasis rather than
   meaning.
5. Name the thing: "the standard library parses the body", not "read back
   with the standard library", and "`Check`", not "the fixture's proof".

Read every comment or paragraph back as a colleague would say it aloud. A
sentence that no one would say is rewritten. The rewrite keeps the facts a
reader needs and drops the facts the code already shows. These sentences
came out of review, each next to what a colleague would write:

| Written                                                    | A colleague writes                                                                 |
| ---------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| The dial is the credential check.                          | Opening a session with the user's credentials is how we check them.                |
| A create is never retried, because it is not a read.       | CreateTable does not retry, because only reads are safe to run twice.              |
| DuckDB runs q as u on u's own DuckDB instance.             | Runs the query on the user's own DuckDB instance.                                  |
| A nil batch is read as no table, not dereferenced.         | A statement without a result set returns a nil batch, which means no tables exist. |
| The trace layer wraps every route; the order is the point. | Add the trace layer last, so it also wraps `/openapi.json`.                        |
