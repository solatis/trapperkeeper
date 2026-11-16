# 0042. The parser of the rule language

Date: 2026-11-14.

## Context

People and an AI client write a rule as text (`adr/0041`). The author of a
faulty text works from the error that the text gets.

Four prototypes parsed the same language: a parser written out by hand,
combinators of our own, winnow 1.0.5 and chumsky 0.14.1. Each prototype ran the
same 37 faulty texts. Each text had a target for the place and the wording of
its error. After tuning, the prototype written out had 1021 lines of code, the
one with combinators of our own 1216, the one with winnow 1300 and the one with
chumsky 1174.

## Decision

The parser is built from parser combinators of our own, with no library
(owner, 2026-11-14).

Of the three kits that were tried, the parser uses the simplest. A choice
tries its alternatives in order, each from the same place. A token test that
does not match notes in one log what it wanted. The parser builds the error
from the notes that lie furthest in the text (owner, 2026-11-14).

No two alternatives of a choice read the same bracketed text. A cap on the
token tests per text refuses a text that would take too long.

The grammar is documented rule by rule, with the form and an example (owner,
2026-11-14).

A tokenizer is a stage of its own before the parser (owner, 2026-11-14).

The parser lives in the engine crate and is always built (owner, 2026-11-14).

It reports the first syntax error and stops (owner, 2026-11-14).

An error holds its place as a range of byte offsets and a reason from a
closed list, and the crate writes the sentence of each reason (owner,
2026-11-14).

The parser holds a text to 4096 characters and to 32 levels of nesting
(owner, 2026-11-14).

A table of faulty texts with the place and the reason of each error is part
of the parser's tests.

## Rejected alternatives

A parser library. In every prototype a good message was code written for that
message. winnow's error holds no place and no text. chumsky gives sets of
expected tokens, and rules of our own turned those sets into sentences.
chumsky adds eleven crates. chumsky's strength is recovery after an error, and
the parser does not use recovery.

A parser written out function by function, which had the best messages before
tuning and the least code. The owner prefers how a grammar reads in
combinators.

A kit in which every combinator merges failures and the grammar marks where a
failure is final. Removing one mark broke no test, so a forgotten one is
silent.

A kit that commits by default, as Parsec does, because it needs marks of its
own and was the largest of the three.

Full parity of error messages with the best prototype. The owner weighs the
simplicity of the code against the clarity of errors, so the parser does not
match the best prototype message for message (owner, 2026-11-14).

A parser over characters, because its errors list single characters.

A cargo feature for the parser, because the tests and CI would cover the
crate twice. A crate of its own, because an SDK could then not print a rule.

Several errors with recovery, because the parser would need code that decides
where to resume.

A line and a column as the place, because they name a point and not a part of
the text.

A message and a hint as two texts, because a caller could only show them and
a test would compare wording.

## Consequences

The kit stays fast only while no two alternatives read the same bracketed
text. That is an invariant of `engine/src/text/AGENTS.md`, and a test and a
cap on the parser's work hold it.

The parser reports a refusal of a grammar rule unless a note of a token test
lies further in the text. The refusal for too much work (`TooComplex`) wins
over any note, because the parse stopped there.

The cap on nesting bounds the parser's stack and not the depth of a tree. The
limits of a tree belong to the validation of the expression. `validate` does
not check the expression yet, so no limit holds a tree.

Whoever shows an error works out the line and the column from the byte range.
Every new reason is a new variant of the public enum `Reason`, which the
crate exports from `engine/src/text`.
