# 0033. The rule format

Date: 2026-11-10.

## Context

The engine crate is the first code that gives a rule a shape. The management
API, the store, the sync wire, the conformance fixtures and every SDK read
and write that shape, so it is decided once, here.

The retired first attempt designed rules as immutable: an edit made a new
rule with a new id, and a test copy of a rule ran beside the live rule under
the `observe` action. The identity decision below continues that design.

## Decision

A rule is read and written as JSON through serde derives on one tree of
structs. The engine has no parser of its own (owner, 2026-11-10). A text form
of a rule comes later, in the web UI only. It converts in both directions,
and a generative test checks the round trip (owner, 2026-11-10).

A rule has a `rule_id`, an optional `rule_title`, an `action`, a
`sample_rate`, a `scope` and `groups`. A rule matches when any of its groups
matches, and a group matches when all of its `conditions` match. A condition
has a `path`, a `field_type`, an `op`, a `value` and the two policies
`on_missing` and `on_coercion_failure`.

A field path is a list of typed segments. A key is `{"key": "order"}`, an
index is `{"index": 0}`, and the wildcard is the string `"*"` (owner,
2026-11-10).

The operators are `eq`, `ne`, `lt`, `le`, `gt`, `ge`, `in`, `not_in`,
`contains`, `starts_with` and `ends_with` (owner, 2026-11-10). `eq` and `ne`
take one scalar of the field type. `lt`, `le`, `gt` and `ge` take one scalar,
on `int`, `float` and `string` fields only. `in` and `not_in` take a list of
scalars of the field type. `contains`, `starts_with` and `ends_with` take one
string, on `string` fields only. An `any` field takes scalars of any type. A
`float` field also takes an int value, and an `int` field refuses a float
value.

A condition's value is a typed scalar, an int, a float, a string or a
boolean, or a list of them. The JSON carries no type tag. A number without a
fraction or an exponent is an int. The engine parses a float to the nearest
f64, so a value reads back as the number that was written.

The limits are constants of the crate (owner, 2026-11-10). A path has at most
8 segments and one wildcard. A membership list has 1 to 32 values. A rule has
at most 64 conditions across all its groups, and the groups themselves are
not capped (owner, 2026-11-10: one cap on the total). The caps on scope tags
are in `adr/0034`.

A rule's identity is its `rule_id` alone, a version 7 UUID that the server
generates (`adr/0020`). An edit is a new rule with a new id (owner,
2026-11-10). The engine never generates an id. It refuses the nil id (owner,
2026-11-11) and every version other than 7. The title is a display name in
the `_title` form of `adr/0020`. It is not part of the identity and may
repeat (owner, 2026-11-10). The engine's rule carries the title, and JSON
omits it when it is absent.

The sample rate is a float fraction in `(0, 1]`, and it is 1 when absent
(owner, 2026-11-10). The actions are `observe`, `drop` and `fail` (owner,
2026-11-10). The policies are those of `adr/0005`, and the field types are
those of `adr/0006`. The scope is an ordered map from tag key to tag value
(`adr/0034`), so a rule serializes to the same bytes every time.

A rule document with a property the model does not have is refused when it
is parsed. This concerns the rule's own JSON and not the fields of a record
that a rule reads. A rule has no schema version field, because nothing is
kept backwards compatible (`principles.md`, 8).

Validation returns every refusal of a rule and does not stop at the first. A
refusal names its place as a JSON pointer (RFC 6901) into the rule document,
such as `/groups/0/conditions/2/path`.

## Rejected alternatives

A text rule language parsed by the engine, because it adds a parser to every
SDK.

A path as a dotted string with brackets, because it needs escaping rules and
a parser in every SDK. A path as a list of plain strings with reserved words,
because a key named `*` or `0` could not be addressed.

A regular expression operator, because Rust and Go use RE2 semantics and
Python does not, so identical results would need a restricted dialect.
Comparison and membership operators alone, because the string tests cover
most data-quality checks at no cost across languages.

String tests on an `any` field, because such a field has no string to test
before coercion. A float value on an `int` field, because `100.0` where an
int is declared is more likely a mistake than a widening.

`serde_json::Value` as the type of a condition's value, because every
consumer would check it again and the parser would accept a nested object.

Limits that the caller passes to validation, because a rule's validity would
then differ per install and the fixtures could pin only the default. No
limits, because an AI client will abuse an unbounded system (owner,
2026-11-10). A cap on groups and a cap on conditions per group, because the
pair dictates a rule's shape and is the easiest to outgrow. A total of 32
conditions, because an allow-list rule with a few alternatives exceeds it
quickly, and a total of 128, because a snapshot of that size weighs on every
event.

A lineage version number beside the id, because the id already changes with
every edit and lineage and rollback are not goals. An opaque identity
string, because the event query would have to parse it. Accepting the nil id
for the convenience of fixtures, because tests must not lower code quality
(owner, 2026-11-11). Accepting any UUID version, because the format would
admit ids the server never issues.

A `rule_name` under the unique-name rule of `adr/0020`, because the test
copy and the live rule exist at the same time. No title in the engine's
rule, because every SDK and the event viewer would then join it back from
the server.

A sample rate of one in N as an integer, because 0.3 cannot be written. A
percentage as an integer, because a rate under one percent cannot be
written.

`observe` as the only action until the sensor exists, because adding a
variant later changes the format after the API shipped.

Accepting and ignoring unknown properties, because a misspelled policy name
would silently become the default.

A schema version field on a rule.

Stopping at the first refusal, because the author would then fix a rule one
round trip at a time. A refusal with a free-text message only, because the
UI could not point at the builder row.

## Consequences

The engine, the server, the web UI and every SDK read the same JSON, and the
conformance fixtures pin the limits. A limit changes by a commit, and the
change invalidates the stored rules that exceed it. A long check becomes
several rules. A rule is verbose to write by hand until the text form
exists. The event query shows a title that may repeat, and the unique-name
rule of `adr/0020` does not apply to rules. Two of the three actions are
executed by nothing until the sensor exists. The engine component cites this
record.
