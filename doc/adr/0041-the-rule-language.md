# 0041. The rule language

Date: 2026-11-14.

## Context

People write rules. The owner wants no new language. The people who write
rules should find the syntax familiar (owner, 2026-11-13).

## Decision

A rule's text is one expression in Python-style syntax that gives a boolean.
The language borrows Python's syntax so that rule authors find it familiar.
Where Python's semantics differ from the semantics that this record and
`adr/0005` define, the language follows the records (owner, 2026-11-13).

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
and `list` (owner, 2026-11-14). The pattern of `re.search`, `re.match` and
`re.fullmatch` is a text literal.

A comparison needs two values of compatible types, in every operator. An int
widens to a float, and nothing else converts. Ordering is for numbers only
(owner, 2026-11-11, 2026-11-13).

A rule declares no types. A literal or an operator fixes the type of the
field it meets. In `a == b`, where both operands are fields, the values in the
data decide the type (owner, 2026-11-13).

There is no truthiness (owner, 2026-11-13).

`any` over an empty list is false, and `all` over an empty list is true
(owner, 2026-11-13).

A NaN that the source treats as a value compares as IEEE 754 defines. Every comparison
with it is false, except `!=`, which is true. An infinity is an ordinary
number (owner, 2026-11-13).

Patterns follow the linear-time dialect of Rust's `regex` crate, which has no
lookaround and no backreferences. The server refuses a pattern outside it
(owner, 2026-11-13).

The engine reads all text as UTF-8. The SDK hands the engine its text as
UTF-8 (owner, 2026-11-12).

A bare name is a field of the record. A key that cannot be a bare name is
written with the word `record` and brackets, as in `record["unit price"]`. A
text in brackets works at every step of a path (owner, 2026-11-13,
2026-11-14).

Twelve words are reserved: `and`, `or`, `not`, `in`, `is`, `if`, `else`, `for`,
`None`, `True`, `False` and `record`. Python's other keywords are names (owner,
2026-11-14). `re` and `math` are reserved too, as the names of the two modules.
A lowercase `null`, `true` or `false` is a name like any other (owner,
2026-11-14). A word of the language never stands after a dot (owner,
2026-11-14).

A rule may run over several lines. A number may hold underscores, and an integer
may be hexadecimal. A membership list may hold fields (owner,
2026-11-14). The parser refuses a comment (owner, 2026-11-13).

A text takes single or double quotes and has a raw form.

`adr/0005` says what a rule does with a value that is not there and with an
operation that cannot be done.

## Rejected alternatives

SQL-style syntax, because SQL operates over many rows, looks JSON keys up
awkwardly and has no standard form for "some element of a list".

CEL, because few data people know it.

A language of our own, because the owner wants no new language.

Declared field types, among them a type `any`, because a literal or an operator
already fixes the type, and in `a == b` the data decides.

Python's answers for a field that holds a null, such as `x != 5` being true
when `x` is not there. The literal `None` follows Python (`adr/0005`).

Truthiness, because it would give a value that is not a boolean a hidden
meaning as a condition. A condition must be a boolean.

Python's full pattern dialect, because a pattern could stall a pipeline.

A `*` in a path, because it cannot say that one element satisfies several
conditions.

Chained comparisons and negative indexes, for now.

The word `row` for the record, because the documents, the events and the API
call it a record, and a dataframe row is one kind of record. Quoted names in
backticks, which need no word for the record and are not Python syntax.

Reserving all of Python's keywords, because a column named `from` would need
a special form.

`re` and `math` as ordinary names, because the parser would then need four
tokens to tell a call such as `re.search(` from a field `re.search`.

Refusing a lowercase `null` or `true` beside a comparison, because a rule may
compare two fields and a real field may be named `null`. The parser cannot
tell `status == null` written by mistake from a comparison of two fields.

Any word after a dot as a key, because the parser would then read the `and`
of the typo `customer. and x > 1` as a key.

Accepting and dropping a comment, because the tree cannot carry a comment, so
the comment would disappear if the database keeps only the tree.

`abs`, for now.

`dict` as a type that `isinstance` tests for, because no rule needs it yet.

All of Python's tests of a text's characters, because `isdecimal` and
`isnumeric` differ from `isdigit` only on unusual digits.

## Consequences

The language has no timestamp type. `decisions.md` holds the open decision on
one. A translator for a host engine walks the tree that the server builds and
the sensors receive (`adr/0040`).
