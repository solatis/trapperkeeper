# 0041. The rule language

Date: 2026-11-14.

## Context

People write rules. The owner wants no new language, and the people who
write rules should find the syntax familiar (owner, 2026-11-13).

## Decision

A rule's text is one expression in Python-style syntax that gives a boolean.
The syntax is borrowed for familiarity, and the semantics are ours where
Python's differ (owner, 2026-11-13).

The language has these kinds of expression (owner, 2026-11-12, 2026-11-13):

- a field and a literal
- the six comparisons, also between a field and a field
- membership, and `in` for a substring and for a list field
- text tests, case and trimming, character classes and patterns
- `and`, `or` and `not`
- `is None` and `isinstance`
- `any` and `all` over a list with a named element
- `len`
- arithmetic
- a conditional expression

Functions and methods come from a closed list. The functions are `len`,
`any`, `all`, `isinstance`, `re.search`, `re.match`, `re.fullmatch` and
`math.isnan` (owner, 2026-11-13). The methods of a text are `startswith`,
`endswith`, `upper`, `lower`, `strip`, `lstrip`, `rstrip`, `isdigit`,
`isalpha`, `isalnum`, `isspace`, `isupper`, `islower` and `isascii` (owner,
2026-11-13, 2026-11-14). `isinstance` tests for `int`, `float`, `str`, `bool`
and `list` (owner, 2026-11-14). The pattern of a pattern function is a
literal.

A comparison needs two values of compatible types, in every operator. An int
widens to a float, and nothing else converts. Ordering is for numbers only
(owner, 2026-11-11, 2026-11-13).

A rule declares no types. A literal or an operator fixes the type, and in
`a == b` the data decides (owner, 2026-11-13).

There is no truthiness (owner, 2026-11-13).

`any` over an empty list is false, and `all` over an empty list is true
(owner, 2026-11-13).

A NaN that the source treats as a value compares as in IEEE. Every comparison
with it is false, except `!=`, which is true. An infinity is an ordinary
number (owner, 2026-11-13).

Patterns follow the linear-time dialect of Rust's `regex` crate, which has no
lookaround and no backreferences. The server refuses a pattern outside it
(owner, 2026-11-13).

Text is UTF-8, and its encoding is the SDK's responsibility (owner,
2026-11-12).

`adr/0005` says what a rule does with a value it cannot use.

## Rejected alternatives

SQL-style syntax, because SQL operates over many rows, looks JSON keys up
awkwardly and has no standard form for "some element of a list".

CEL, because few data people know it.

A language of our own.

Declared field types and an `any` type.

Python's answers where they surprise, such as `None != 5` being true.

Truthiness.

Python's full pattern dialect, because a pattern could stall a pipeline.

A `*` in a path, because it cannot say that one element satisfies several
conditions.

Chained comparisons and negative indexes, for now.

`abs`, for now.

`dict` as a type that `isinstance` tests for, because no rule needs it yet.

All of Python's tests of a text's characters, because `isdecimal` and
`isnumeric` differ from `isdigit` only on unusual digits.

## Consequences

Timestamps wait (`decisions.md`). A translator for a host engine walks the
same tree (`adr/0040`). The details of the grammar are fixed when the parser
is written.
