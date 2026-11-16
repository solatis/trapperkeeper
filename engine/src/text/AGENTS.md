# engine/src/text/

**What**: the module turns the text of a rule into the expression tree and
prints a tree back as rule text. It holds the tokenizer, the kit of parser
combinators, the grammar written with the kit, the error that a faulty text
gets, and the printer.

**Map**:

| File         | What                                                                                                                    | When to read                                                                                                                                                                                           |
| ------------ | ----------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `mod.rs`     | the files of the module and what it exports                                                                             | what the crate can reach of the parser                                                                                                                                                                 |
| `error.rs`   | the syntax error, its reasons and their sentences                                                                       | why a text was refused; adding a reason; the wording of an error                                                                                                                                       |
| `tokens.rs`  | the tokenizer and the kinds of token                                                                                    | what a number, a text or an operator looks like; an error about one character                                                                                                                          |
| `kit.rs`     | the parser combinators, the log of what was wanted and `explain`                                                        | how a choice picks the error it reports; adding a combinator; where the caps on parse steps and nesting refuse a text                                                                                  |
| `grammar.rs` | `parse`, the rules of the grammar, one per level of precedence, and the closed lists `FUNCTIONS`, `METHODS` and `TYPES` | the precedence of an operator; adding a function, a method or a keyword; an error for one named mistake; the "did you mean" guess for a misspelled name; how a bare name becomes a field or an element |
| `print.rs`   | `Display` for a tree, which writes it as rule text                                                                      | where a printed rule gets parentheses or brackets                                                                                                                                                      |

**Invariants**:

- The tokenizer does not fail. Its error is the last token, and the parser
  reports it only when it gets there.
- No two alternatives of a choice read the same bracketed text, or the
  parser's time doubles per level of nesting. Among the alternatives of
  `atom`, `call` is the only one that reads a name and its `(`. The test
  `a_deep_faulty_text_is_refused_for_its_own_mistake` in
  `engine/tests/text.rs` fails when two alternatives read the same bracket,
  because the doubled work reaches `MAX_PARSE_STEPS` and the error becomes
  `TooComplex` in place of the mistake.
- A token that may follow almost any value is `hidden`, or it shows up in
  every "expected" error.
- Every form that makes the grammar call itself goes through `nested` or
  `delimited`. `nested` checks `MAX_NESTING` before it runs the body, and
  `delimited` goes through `nested`. A form that bypasses both escapes
  `MAX_NESTING`.
- Every rule of the grammar says its form and gives an example of the text it
  reads, and every alternative of a choice says which text it is for. Every
  combinator of the kit shows what it does on an example.
- A new reason gets a faulty text with its sentence in `faulty`, in
  `engine/tests/text.rs`. No test checks that every reason has a row in
  `faulty`, so the sentence of a reason without a row is never run by a test.
- A canonical tree is a tree that the parser can build, and it prints to a
  text that parses back to the same tree. The property
  `a_printed_tree_parses_back_to_itself` in `engine/tests/text.rs` checks
  this over trees from `canonical_in`. A change to the grammar or to the
  printer keeps that property passing.

**Why**:

- How the parser is built, and what lost:
  `doc/adr/0042-the-parser-of-the-rule-language.md`.

**How to**:

- `cargo test -p trapperkeeper-engine --test text` runs the tables `VALID`
  and `faulty` and the properties of the `proptest!` block, all in
  `engine/tests/text.rs`.
- Adding a method: a variant of `Expr` in `engine/src/expr.rs` with its arm
  in `Expr::children_mut`, an entry in `UNARY` or `BINARY` of
  `engine/tests/common/mod.rs`, which the strategy `expr` draws from, a row
  in `METHODS`, an arm in the printer, and an entry in `UNARY` or `BINARY` of
  `engine/tests/text.rs`, which `canonical_in` draws from.
