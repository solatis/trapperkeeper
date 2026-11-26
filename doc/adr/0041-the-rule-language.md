# 0041. The rule language

Date: 2026-11-26.

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
`any`, `all`, `isinstance`, `re.search`, `re.match` and `re.fullmatch` (owner,
2026-11-13, 2026-11-26). The methods of a text are `startswith`,
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

A NaN is a null (`adr/0005`), and an infinity is an ordinary number (owner,
2026-11-13, 2026-11-26).

Patterns follow the linear-time dialect of Rust's `regex` crate, which has no
lookaround and no backreferences. The server refuses a pattern outside it
(owner, 2026-11-13). Validation compiles each pattern with the `regex` crate
and its defaults, and it refuses a pattern that does not compile, with the
crate's own error text (owner, 2026-11-16). The dialect is whatever those
defaults accept (owner, 2026-11-17).

The defaults give a pattern Unicode classes, as in Python, and a cap on its
compiled size, 10 MiB in `regex` 1.13. Validation sets no cap of its own
(owner, 2026-11-16).

The engine reads text in the encoding that its source holds, which is UTF-8,
or UTF-32 for the fixed-width cells of numpy's `U` type. It converts neither
(owner, 2026-11-21, 2026-11-26). A loop compares text in the encoding of its
column, and the planner encodes each literal of a rule in both encodings when
it plans (owner, 2026-11-26). Two UTF-8 texts are equal exactly when their code
points are, so equality needs no decoding.

A bare name is a field of the record. A key that cannot be a bare name is
written with the word `record` and brackets, as in `record["unit price"]`. A
text in brackets works at every step of a path (owner, 2026-11-13,
2026-11-14).

Twelve words are reserved: `and`, `or`, `not`, `in`, `is`, `if`, `else`, `for`,
`None`, `True`, `False` and `record`. Python's other keywords are names (owner,
2026-11-14). `re` is a thirteenth reserved word, as the name of the module of
the pattern functions, so a field `re` is written `record["re"]` (owner,
2026-11-26). A lowercase `null`, `true` or `false` is a name like any other
(owner, 2026-11-14). A word of the language never stands after a dot (owner,
2026-11-14).

A rule may run over several lines. A number may hold underscores, and an integer
may be hexadecimal. A membership list may hold fields (owner, 2026-11-14). The
parser refuses a comment (owner, 2026-11-13).

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

Checking each pattern with `regex-syntax` alone, the crate's pattern parser,
without compiling it, because the parser accepts a pattern that is too large to
compile, such as `(a{1000}){1000}`.

Converting a `U` column to UTF-8, in the adapter or per chunk in the core. The
owner gave no reason beyond the decision to read the cells in place.

UTF-32 for every text, because nearly every source holds UTF-8 and would be
converted per chunk.

Compiling every pattern with the crate's Unicode mode off, which gives `\w` and
`\d` ASCII classes, because the crate then refuses ordinary patterns on text,
such as `.` and `\D+`. Without Unicode mode such a pattern can match a byte that
is not UTF-8, and the crate's `Regex` matches only on text.

Refusal reasons of our own for lookaround and for backreferences, and a byte
range inside the pattern in place of the crate's message, because one reason
that holds the crate's text needs the least code, and the text of a syntax
error already marks the place inside the pattern.

A constant of ours for the cap on a compiled pattern, because the crate's own
default is the cap wanted here, and the crate exports no constant that ours
could name.

A test with a table of patterns that pins which forms the dialect accepts and
refuses, because the forms of the dialect do not matter here, and safety,
performance and stability count for more. No test of ours then shows when a new
version of the crate changes which patterns validate.

A `*` in a path, because it cannot say that one element satisfies several
conditions.

Chained comparisons and negative indexes, for now.

The word `row` for the record, because the documents, the events and the API
call it a record, and a dataframe row is one kind of record. Quoted names in
backticks, which need no word for the record and are not Python syntax.

Reserving all of Python's keywords, because a column named `from` would need
a special form.

`re` as an ordinary name. The record gives no reason against it.

`math.isnan`, because a NaN is a null and `x is None` finds it.

`math` kept reserved for a later function, because a field named `math` would
need brackets until then.

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

A counted repeat of a Unicode class is large and slow. `^\w{1,64}$` compiles to
about 3 MB and matched about 38 times slower than its ASCII form, measured as a
ratio on the machine that `adr/0035` names. An author writes `(?-u:\w)` for the
ASCII class inside one pattern. The dialect and the cap on a compiled pattern
change when the `regex` crate changes its defaults. The engine crate depends on
`regex`, and so does every SDK.

Every loop over text has a UTF-8 form and a UTF-32 form. The `regex` crate
matches UTF-8 only, so how a pattern runs on a UTF-32 column is open
(`decisions.md`).

The language has no timestamp type. `decisions.md` holds the open decision on
one. A translator for a host engine walks the tree that the server builds and
the sensors receive (`adr/0040`).
