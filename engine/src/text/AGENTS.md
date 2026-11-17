# engine/src/text/

**What**: the module turns the text of a rule into the expression tree,
`Expr` in `engine/src/expr.rs`, and prints a tree back as rule text. It holds
the tokenizer, the kit of parser combinators, the grammar written with the
kit, the error that a faulty text gets, and the printer.

**Map**:

| File         | What                                                                                                                    | When to read                                                                                                                                                                                                                          |
| ------------ | ----------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `mod.rs`     | the files of the module and what it exports                                                                             | which items of the module a user of the crate can name                                                                                                                                                                                |
| `error.rs`   | the syntax error, its reasons and their sentences                                                                       | why a text was refused; adding a reason; the wording of an error                                                                                                                                                                      |
| `tokens.rs`  | the tokenizer and the kinds of token                                                                                    | what a number, a text or an operator looks like; an error about one character                                                                                                                                                         |
| `kit.rs`     | the parser combinators, the log of what was wanted and `explain`                                                        | how a choice picks the error it reports; adding a combinator; where the caps on parse steps and nesting refuse a text                                                                                                                 |
| `grammar.rs` | `parse`, the rules of the grammar, one per level of precedence, and the closed lists `FUNCTIONS`, `METHODS` and `TYPES` | the precedence of an operator; adding a function, a method or a keyword; an error for one named mistake; the "did you mean" guess for a misspelled name; how a bare name becomes a field of the record or the element of a quantifier |
| `print.rs`   | `Display` for a tree, which writes it as rule text                                                                      | where a printed rule gets parentheses or brackets                                                                                                                                                                                     |

**Invariants**:

- The tokenizer does not fail. Its error is the last token, a `Bad` token
  that holds the reason, and the parser reports it only when it gets there.
- No two alternatives of a choice read the same bracketed text, or the
  parser's time doubles per level of nesting. Each level would then read its
  inside twice, and the inside holds the next level. Among the alternatives of
  `atom`, `call` is the only one that reads a name and its `(`. The test
  `a_deep_faulty_text_is_refused_for_its_own_mistake` in
  `engine/tests/text.rs` fails when two alternatives read the same bracketed
  text, because the doubled work reaches `MAX_PARSE_STEPS` and the error becomes
  `TooComplex` in place of the mistake.
- A token that may follow almost any value is read through `hidden`, the
  combinator of `kit.rs` that keeps a token test out of errors, or it shows
  up in every "expected" error.
- Every form that makes the grammar call itself goes through `nested` or
  `delimited`: a bracketed form, the operand of `not` or of a minus, and the
  branch after `else`. `nested` checks `MAX_NESTING` before it runs the body,
  and `delimited` goes through `nested`. A form that bypasses both escapes
  `MAX_NESTING`.
- Every rule of the grammar says its form and gives an example of the text it
  reads, and every alternative of a choice says which text it is for. Every
  combinator of the kit shows what it does on an example.
- A new case of `Reason` gets a row in the table `faulty` of
  `engine/tests/text.rs`: a faulty text, the place of its error, the reason
  and its sentence. `TooComplex` is the one exception. No text reaches it
  while no two alternatives of a choice read the same bracketed text, and the
  test `a_deep_faulty_text_is_refused_for_its_own_mistake` guards that rule.
  No test checks that every other reason has a row in `faulty`, so the
  sentence of a reason without a row is never run by a test.
- A canonical tree is a tree that the parser can build, and it prints to a
  text that parses back to the same tree, as long as the printed text stays
  within the parser's caps on length and nesting. The printer writes spaces
  and parentheses that an author can leave out, so a printed text can be
  longer than the text it came from. An `and` directly under an `and`
  is a tree that the parser cannot build. The property
  `a_printed_tree_parses_back_to_itself` in `engine/tests/text.rs` checks
  this over the trees that the strategy `canonical_in` generates. A change to
  the grammar or to the printer keeps that property passing.

**Why**:

- How the parser is built, and what lost:
  `doc/adr/0042-the-parser-of-the-rule-language.md`.

**How to**:

- `cargo test -p trapperkeeper-engine --test text` runs the tables `VALID`
  and `faulty` and the properties of the `proptest!` block, all in
  `engine/tests/text.rs`.
- Adding a method: a variant of `Expr` in `engine/src/expr.rs` with its arm
  in `Expr::children_mut`, a row in `METHODS`, an arm in the printer, and
  one entry in each of the test files `engine/tests/common/mod.rs` and
  `engine/tests/text.rs`. Each file has a list `UNARY` for a method without
  an argument and a list `BINARY` for a method with one. The lists of
  `engine/tests/common/mod.rs` feed the strategy `expr`, which generates any
  tree for the properties of the JSON form. The lists of
  `engine/tests/text.rs` feed `canonical_in`, which generates only trees that
  the parser can build.
