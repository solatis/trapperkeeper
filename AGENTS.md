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
[all|placement|narrative] [check] [paths...]` is the repair path; it applies
or checks all of the above after the fact, and its worked example is in
`.claude/skills/doc-discipline/narrative.md`.
