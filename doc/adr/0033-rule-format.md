# 0033. The rule format

Date: 2026-11-14.

## Context

The management API, the store, the sync wire, the conformance fixtures and
every SDK read and write the shape of a rule, so it is decided once, here.

The retired first attempt designed rules as immutable: an edit made a new
rule with a new id, and a test copy of a rule ran beside the live rule under
the `observe` action. The identity decision below continues that design.

## Decision

A rule has a `rule_id`, an optional `rule_title`, an `action`, a
`sample_rate`, a `scope` and one expression.

The expression is written as text in the rule language (`adr/0041`). A
planner reads it as a tree (owner, 2026-11-13).

The tree is written in JSON with one key that names the kind of each node, as
in `{"gt": [{"field": ["amount"]}, {"int": 100}]}` (owner, 2026-11-13). The
tree keeps the operators as the author wrote them (owner, 2026-11-14). A
field is one node with a path of keys and indexes (owner, 2026-11-14). `and`
and `or` hold a list of operands. The pattern of a pattern function is a text
in its node. The engine's rule holds the tree and not the text, because the
text derives from the tree (owner, 2026-11-14).

The actions are `observe`, `drop` and `throw`. `throw` raises into the
pipeline. A rule has one action (owner, 2026-11-11, 2026-11-13).

The server parses the text and validates the rule. No SDK validates (owner,
2026-11-11). A sensor receives the tree. The owner does not want this set in
stone and may change it if it proves inconvenient (owner, 2026-11-13). A
sensor checks what it receives against corruption and forgery, and it returns
an error on anything unexpected (owner, 2026-11-12). Whether the database
stores the text, the tree or both is open (`decisions.md`).

Sampling happens above the evaluation of a rule, and a rule that is sampled
out is absent (owner, 2026-11-12).

The limits are constants of the crate, and no caller sets them (owner,
2026-11-10). The caps on scope tags are in `adr/0034`.

A rule's identity is its `rule_id` alone, a version 7 UUID that the server
generates (`adr/0020`). An edit is a new rule with a new id (owner,
2026-11-10). The engine never generates an id. It refuses the nil UUID (owner,
2026-11-11) and every UUID version other than 7. The title is a display name in
the `_title` form of `adr/0020`. It is not part of the identity and may
repeat (owner, 2026-11-10). The engine's rule carries the title, and JSON
omits it when it is absent.

The sample rate is a float fraction in `(0, 1]`, and it is 1 when absent
(owner, 2026-11-10). The scope is an ordered map from tag key to tag value
(`adr/0034`), so a rule serializes to the same bytes every time.

A rule document with a property the model does not have is refused when it
is parsed. This concerns the rule's own JSON and not the fields of a record
that a rule reads. A rule has no schema version field, because nothing is
kept backwards compatible (`principles.md`, 8).

Validation returns every refusal of a rule and does not stop at the first.
Each refusal names its place.

## Rejected alternatives

The name `fail` for the action that raises, because `throw` says what it
does.

A rule as an OR of groups of conditions, because it cannot compare a field
with a field or nest AND and OR.

A `kind` property in every node, because it is the longest form and a reader
has to buffer a node to find its kind. Nested arrays, because the reader and
the writer would be written by hand.

A small core of operators that the parser lowers every form to, because a
printed rule would read differently from what was written.

A field as nested nodes, because the planner would fold them back into paths.

The author's text beside the tree, because something would have to check that
the two say the same.

`and` and `or` as pairs, because a rule of many conditions would nest deeper
than a JSON reader accepts.

Sending the text to a sensor, because a sensor would then parse at every sync
and could meet a text that its version of the parser does not accept.
Validation in an SDK, because an SDK executes rules that the server
returned.

Limits that the caller passes to validation, because a rule's validity would
then differ per install. No limits, because an AI client will abuse an
unbounded system (owner, 2026-11-10).

A lineage version number beside the id, because the id already changes with
every edit and lineage and rollback are not goals. An opaque identity
string, because the event query would have to parse it. Accepting the nil UUID
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

Accepting and ignoring unknown properties, because a misspelled property
would silently become the default.

A schema version field on a rule.

Stopping at the first refusal, because the author would then fix a rule one
round trip at a time. A refusal with a free-text message only, because the
UI could not point at the place of the error.

## Consequences

The web UI edits a rule as text. The engine, the server and every SDK read
the same tree. A limit changes by a commit, and the change invalidates the
stored rules that exceed it. The event query shows a title that may repeat,
and the unique-name rule of `adr/0020` does not apply to rules. Two of the
three actions are executed by nothing until the sensor exists. The engine
component cites this record.
