# 0042. The parser of the rule language

Date: 2026-11-14.

## Context

A rule is written as text by people and by an AI client (`adr/0041`), and the
error a faulty text gets is what its author works from.

Four prototypes parsed the same language and ran the same 37 faulty texts,
each with a target for the place and the wording of its error: a parser
written out by hand, combinators of our own, winnow 1.0.5 and chumsky 0.14.1.

## Decision

The parser is built from parser combinators of our own, with no library
(owner, 2026-11-14).

The kit is the simplest one. A choice tries its alternatives in order, each
from the same place. One log of what each token test wanted gives the error,
from the notes furthest in the text (owner, 2026-11-14).

No two alternatives read the same bracketed text, and a cap on the parser's
work refuses a text that would take too long.

The grammar is documented rule by rule, with the form and an example (owner,
2026-11-14).

A tokenizer is a stage of its own before the parser (owner, 2026-11-14).

An error holds its place as a range of byte offsets and a reason from a
closed list, and the crate writes the sentence of each reason (owner,
2026-11-14).

The parser holds a text to 4096 characters and to 32 levels of nesting
(owner, 2026-11-14).

## Rejected alternatives

A parser library. In every prototype a good message was code written for that
message. winnow's error holds no place and no text. chumsky gives sets of
expected tokens that rules of our own turned into sentences, it adds eleven
crates, and its strength is recovery after an error, which the parser does
not use.

A parser written out function by function, which had the best messages before
tuning and the least code. The owner prefers how a grammar reads in
combinators.

A kit in which every combinator merges failures and the grammar marks where a
failure is final. Removing one mark broke no test, so a forgotten one is
silent.

A kit that commits by default, as Parsec does, because it needs marks of its
own and was the largest of the three.

Full parity of error messages with the best prototype. Code simplicity is
balanced against the clarity of errors (owner, 2026-11-14).

A parser over characters, because its errors list single characters.

A line and a column as the place, because they name a point and not a part of
the text.

A message and a hint as two texts, because a caller could only show them and
a test would compare wording.

## Consequences

The kit stays fast only while no two alternatives read the same bracketed
text. That is an invariant of `engine/src/text/AGENTS.md`, and a test and a
cap on the parser's work hold it.

A refusal is reported only when no plainer failure lies further in the text.

The cap on nesting bounds the parser's stack and not the depth of a tree.
Validation holds the tree to its own limits.

Measured on the same texts after tuning, in lines of prototype code: 1021
written out, 1216 with combinators of our own, 1300 with winnow, 1174 with
chumsky.

Whoever shows an error works out the line and the column from the byte range.
Every new message is a new case of a public type.
