# doc/

Knowledge that has no home in code, or none yet. Everything else belongs in the
`AGENTS.md` of the directory that implements it (see the root `AGENTS.md`).

| File              | What                                           | When to read                       |
| ----------------- | ---------------------------------------------- | ---------------------------------- |
| `overview.md`     | Product, problem, users, vocabulary, non-goals | First contact; naming things       |
| `architecture.md` | Components, boundaries, end-to-end flows       | Adding or changing a component     |
| `principles.md`   | Trade-offs we resolve the same way every time  | Any design choice with a trade-off |
| `decisions.md`    | Open decisions, and settled ones awaiting code | Before re-deciding something       |

## Rules

These rules come from the initial setup. They are refined incrementally as the
project grows, like everything else; change one when it gets in the way.

1. Budget: these four files, about 200 lines each, no subfolders. A need for
   a fifth file means the content belongs next to code.
2. Eviction: when a component directory is created, its section in
   `architecture.md` shrinks to one paragraph plus a pointer, and its entries
   in `decisions.md` move into the component's `AGENTS.md` under **Why**.
   What moved is deleted here; the one-paragraph section and its pointer
   stay. A section shrinks when the code of that role lands, not when a
   directory that will later hold it is created (owner, 2026-10-31).
3. `decisions.md` is a holding pen, not an archive. No numbering, no status
   history, no superseded entries. An entry exists only while it has no code
   home and says something `principles.md` and `architecture.md` do not.
   Expect it to shrink.
4. Language, framework and library choices sit in `decisions.md` only until
   code carries them, then move to the component that made them, or to the
   root `AGENTS.md` when they apply repo-wide.
5. No doc tooling: no frontmatter, no indexes, no linters, no templates. The
   `AGENTS.md` hierarchy is the index.
