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
| What is decided but not built, what is open   | `doc/decisions.md`      |
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
| `engine/`      | the rule engine crate, embeddable by pipelines     | with M3                      |
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
  status checks of the `master` ruleset, which the owner keeps by hand.

## Why

- Rust for the server side: the web server and the APIs are written in Rust.
  Performance is extremely important for this project; Rust is correct by
  construction where a garbage-collected language such as Go is less suited to
  FFI-style use, which keeps the shared evaluator core option open; the
  owner's background is heavily C++ and Haskell; and Rust is gaining ground
  for this kind of project with LLM providers and the wider community
  (Anthropic, OpenAI, Cloudflare, the Linux kernel). SDK languages are a
  separate decision (see `doc/overview.md`, Integration surface).
- Latest stable Rust only, no minimum version (owner, 2026-10-31): code may
  use any feature as soon as it is stable, and CI builds with the current
  stable toolchain. Cost: anyone building TrapperKeeper needs an up-to-date
  toolchain.
- Monorepo: all components are developed in this one repository, if at all
  possible. Cross-cutting changes are expected to be common and are far easier
  to manage as a single PR, and what is released together belongs in one
  repository; all components are released together. Exception: an SDK
  language whose publishing requires a standalone repository, decided case by
  case.
- One top-level directory per component (owner, 2026-10-31). Rejected:
  grouping the Rust crates under `crates/`, because each component keeps its
  `AGENTS.md` beside its code and the Rust sensor SDK belongs with the other
  SDKs.
- Clippy pedantic, denied in the workspace lints table (owner, 2026-10-31).
  Rejected: the default group alone, as too lenient; and warning locally while
  denying only in CI, because a commit should fail the same way in both
  places.
- SDK order: Rust first, then Python, then Go. Later languages (Java, Node.js
  and others) are undecided, and Node.js comes last. They span several
  milestones (owner, 2026-10-31).

## Workflow

The owner is the repository's maintainer, the only person who opens a stage
or answers a question. "The question tool" is AskUserQuestion.

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
   - **Why**: decisions that shaped this component, a few lines each.
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

Comment density tracks complexity. Trivial functions get none. A function that
captures business logic or is an algorithm is written so that its comments
alone tell the story:

1. The doc comment above the function is the contract for callers. No
   implementation detail.
2. The top of the body describes the process: the strategy, why it is shaped
   this way, and the steps, numbered when they form a sequence.
3. Each step in the code has a comment stating what it achieves or why, in one
   line where that fits. Never a paraphrase of the syntax.
4. Constants say where they came from, impossible branches say why they are
   impossible, hot or odd constructs say what was rejected.
5. Only state what is known. When the owner is in the session, an unknown
   reason, or a function you cannot place under the rules above, is a
   question to the owner through the question tool, asked before the commit
   that needs it. Never invent a reason and never drop one silently.
6. After writing a function's comments, read them back top to bottom as
   separate claims and check each against the code below it. Fix or delete
   any claim the code does not bear out.

This is the primary path: new code is written this way. `/doc-discipline
[all|placement|narrative|read] [check] [paths...] [plan.md]` is the repair
path; it applies or checks all of the above after the fact, `read` loads the
shape before code or a plan is written, and a plan path reconciles the code
against the reasons the plan promised. Its worked example is in
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
sentence that no one would say is rewritten. The facts it carried are kept,
and nothing is added.
