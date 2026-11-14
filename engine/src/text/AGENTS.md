# engine/src/text/

**What**: a rule as text. The module holds the tokenizer, the kit of parser
combinators, the grammar written with the kit, the error that a faulty text
gets, and the printer.

**Map**:

| File         | What                                                              | When to read                                                                              |
| ------------ | ----------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| `mod.rs`     | the files of the module and what it exports                       | what the crate can reach of the parser                                                    |
| `error.rs`   | the syntax error, its reasons and their sentences                 | why a text was refused; adding a reason; the wording of an error                          |
| `tokens.rs`  | the tokenizer and the kinds of token                              | what a number, a text or an operator looks like; an error about one character             |
| `kit.rs`     | the parser combinators, the log of what was wanted and `explain`  | how a choice picks the error it reports; adding a combinator                              |
| `grammar.rs` | `parse` and the rules of the grammar, one per level of precedence | the precedence of an operator; adding a function, a method or a keyword; a targeted error |
| `print.rs`   | `Display` for a tree, which writes it as rule text                | where a printed rule gets parentheses or brackets                                         |

**Invariants**:

- The tokenizer does not fail. Its error is the last token, and the parser
  reports it only when it gets there.
- The kit carries no failure in its replies. A token test that does not match
  notes what it wanted, and the error comes from the notes furthest in the
  text. A refusal is the error unless a note lies further.
- No two alternatives of a choice read the same bracketed text, or the
  parser's time doubles per level of nesting. `call` is the only rule that
  reads a name and its `(`. The test of deep faulty texts fails when this
  breaks.
- A token that may follow almost any value is `hidden`, or it shows up in
  every "expected" error.
- Every form that makes the grammar call itself goes through `nested` or
  `delimited`, or the cap on nesting does not hold.
- Every rule of the grammar says its form and gives an example of the text it
  reads, and every alternative of a choice says which text it is for. Every
  combinator of the kit shows what it does on an example. The cut pass of the
  comment rules leaves these comments.
- A new reason gets a faulty text with its sentence in `faulty`, in
  `engine/tests/text.rs`. The test fails for a reason without a row.
- A printed canonical tree parses back to itself. A change to the grammar or
  to the printer keeps that property green.

**Why**:

- How the parser is built, and what lost:
  `doc/adr/0042-the-parser-of-the-rule-language.md`.
- The grammar's decisions: `doc/adr/0041-the-rule-language.md`.

**How to**:

- `cargo test -p trapperkeeper-engine --test text` runs the properties and
  the tables.
- Adding a method: a variant of `Expr`, an entry in the strategy `expr` of
  `tests/common/mod.rs`, a row in `METHODS`, an arm in the printer, and a
  case in `canonical_in`.
