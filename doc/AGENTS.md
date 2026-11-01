# doc/

Knowledge that has no home in code, or none yet. Everything else belongs in the
`AGENTS.md` of the directory that implements it (see the root `AGENTS.md`).

| File              | What                                               | When to read                            |
| ----------------- | -------------------------------------------------- | --------------------------------------- |
| `overview.md`     | Product, problem, users, vocabulary, non-goals     | First contact; naming things            |
| `architecture.md` | Components, boundaries, end-to-end flows           | Adding or changing a component          |
| `principles.md`   | Trade-offs we resolve the same way every time      | Any design choice with a trade-off      |
| `decisions.md`    | Open decisions, not yet made                       | Before deciding something               |
| `adr/`            | Decision records: what was decided, why, what lost | Before re-deciding; see `adr/AGENTS.md` |

## Rules

These rules come from the initial setup. They are refined incrementally as the
project grows, like everything else; change one when it gets in the way.

1. Budget: the four files, about 200 lines each, and no subfolder other than
   `adr/`. A need for a fifth file means the content belongs next to code.
2. Eviction: when the code of a role described in `architecture.md` lands, the
   role's section shrinks to one paragraph plus a pointer to the component's
   `AGENTS.md`. The shrink happens when the code lands, not when a directory
   that will later hold it is created (owner, 2026-10-31). What moved is
   deleted here; the one-paragraph section and its pointer stay.
3. `decisions.md` holds open decisions only. A decision, once made, becomes a
   record in `adr/` and leaves `decisions.md`. A changed decision rewrites its
   record in place; there is no historical trail of decisions (owner,
   2026-11-01).
4. Language, framework and library choices are records in `adr/`. The
   component that carries one cites the record in its `AGENTS.md` under
   **Why**, and a code comment that needs the reason points at the record.
5. No doc tooling: no frontmatter, no indexes, no linters, no templates. The
   `AGENTS.md` hierarchy is the index.
