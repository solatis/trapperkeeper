# 0042. The parser of the rule language

Date: 2026-11-14.

## Context

A rule is written as text by people and by an AI client (`adr/0041`), and the
error a faulty text gets is what its author works from.

Four prototypes parsed the same language and ran the same 37 faulty texts,
each with a target for the place and the wording of its error: a parser
written out by hand, combinators of our own, winnow 1.0.5 and chumsky 0.14.1.

## Decision

A tokenizer is a stage of its own before the parser (owner, 2026-11-14).

An error holds its place as a range of byte offsets and a reason from a
closed list, and the crate writes the sentence of each reason (owner,
2026-11-14).

The parser holds a text to 4096 characters and to 32 levels of nesting
(owner, 2026-11-14).

## Rejected alternatives

A parser over characters, because its errors list single characters.

A line and a column as the place, because they name a point and not a part of
the text.

A message and a hint as two texts, because a caller could only show them and
a test would compare wording.

## Consequences

Whoever shows an error works out the line and the column from the byte range.
Every new message is a new case of a public type.
