# engine/src/text/

**What**: a rule as text. The module holds the tokenizer and the error that a
faulty text gets.

**Map**:

| File        | What                                              | When to read                                                                  |
| ----------- | ------------------------------------------------- | ----------------------------------------------------------------------------- |
| `mod.rs`    | the files of the module and what it exports       | what the crate can reach of the parser                                        |
| `error.rs`  | the syntax error, its reasons and their sentences | why a text was refused; adding a reason; the wording of an error              |
| `tokens.rs` | the tokenizer and the kinds of token              | what a number, a text or an operator looks like; an error about one character |

**Invariants**:

- The tokenizer does not fail. Its error is the last token, and the parser
  reports it only when it gets there.

**Why**:

- How the parser is built, and what lost:
  `doc/adr/0042-the-parser-of-the-rule-language.md`.
