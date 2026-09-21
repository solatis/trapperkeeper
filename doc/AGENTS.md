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

1. Budget: these four files, about 200 lines each, no subfolders. A need for
   a fifth file means the content belongs next to code.
2. Eviction: when a component directory is created, its section in
   `architecture.md` shrinks to one paragraph plus a pointer, and its entries
   in `decisions.md` move into the component's `AGENTS.md` under **Why**. Both
   are deleted here.
3. `decisions.md` is a holding pen, not an archive. No numbering, no status
   history, no superseded entries. An entry exists only while it has no code
   home and says something `principles.md` and `architecture.md` do not.
   Expect it to shrink.
4. Nothing in `doc/` names an implementation language, framework or library.
   Such choices are recorded only inside the component that made them. Target
   SDK languages and supported pipeline frameworks are product facts and are
   fine.
5. No doc tooling: no frontmatter, no indexes, no linters, no templates. The
   `AGENTS.md` hierarchy is the index.
