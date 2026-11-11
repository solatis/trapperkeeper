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
| `sdk/<lang>/`  | one sensor SDK per language                        | Rust with M6, Python with M8 |
| `web/`         | the browser UI                                     | with M7                      |
| `conformance/` | the language-neutral evaluation fixtures           | with M3                      |

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

A comment says what the code cannot say for itself. Most functions need a
short doc comment and a few inline comments, and many need none at all.

1. The doc comment above a function is the contract for callers, in one to
   three sentences: what it does, what it returns and how it fails. It holds
   no implementation detail.
2. A reason sits inline at the line it explains, and it appears once. An
   inline comment is one line, two at most. Every line that would make a
   colleague stop and ask "why?" gets one, in any function, tests included.
3. A function whose steps must run in an order the code does not make
   obvious starts its body with an overview that lists the steps, one
   numbered line each. The overview carries no reasons. The reasons sit at
   the steps, and each step comment starts with the step's number.
4. Constants say where they came from, and impossible branches say why they
   are impossible. A construct that looks wrong says which alternative was
   rejected, but only where a reader of that line would propose the
   alternative. Other rejected alternatives belong in `doc/adr/` or nowhere.
5. A comment describes the code as it is. When code changes, rewrite every
   comment block the change touches from the new code, as if the code had
   always worked this way. Write the new block before rereading the old one,
   then carry over only the facts the code cannot show. A comment never
   mentions an earlier version, a plan, a review, an issue, a unit of work
   or the conversation that shaped it.
6. Delete a comment that an experienced developer in the language would not
   miss. Comments stay well under half of a function body's lines.
7. Only state what is known. When the owner is in the session, an unknown
   reason, or a function you cannot place under the rules above, is a
   question to the owner through the question tool, asked before the commit
   that needs it. Never invent a reason.
8. After writing a function's comments, read them back top to bottom as
   separate claims and check each against the code below it. Fix or delete
   any claim the code does not bear out.

This is the primary path: new code is written this way. `/doc-discipline
[all|placement|narrative|read] [check] [paths...] [plan.md]` is the repair
path. It applies or checks all of the above after the fact, `read` loads the
shape before code or a plan is written, and a plan path reconciles the code
against the reasons the plan promised. Its worked examples are in
`.claude/skills/doc-discipline/narrative.md`.

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
