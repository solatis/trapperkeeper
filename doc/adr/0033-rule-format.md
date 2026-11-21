# 0033. The rule format

Date: 2026-11-17.

## Context

The management API, the store, the sync wire, the conformance fixtures and
every SDK read and write the shape of a rule, so it is decided once, here.

An operator tests a change to a rule with a copy of the rule that runs beside
the live rule under the `observe` action.

## Decision

A rule has a `rule_id`, an optional `rule_title`, an `action`, a
`sample_rate`, a `scope` and one expression.

The expression is written as text in the rule language (`adr/0041`). A
planner reads it as a tree (owner, 2026-11-13).

The tree is written in JSON with one key that names the kind of each node, as in
`{"gt": [{"field": ["amount"]}, {"int": 100}]}` (owner, 2026-11-13). The tree
keeps the operators as the author wrote them (owner, 2026-11-14). A field is one
node with a path of keys and indexes (owner, 2026-11-14). `and` and `or` hold a
list of operands. The pattern of `re.search`, `re.match` and `re.fullmatch` is a
text in its node. The `Rule` type of the engine crate holds the tree and not the
text, because the text can be printed from the tree (owner, 2026-11-14).

The actions are `observe`, `drop` and `throw`. `throw` raises into the
pipeline. A rule has one action (owner, 2026-11-11, 2026-11-13).

The server parses the text and validates the rule. No SDK validates (owner,
2026-11-11). A sensor receives the tree. The owner may change the choice that
the server parses the text and a sensor receives the tree, if that choice proves
inconvenient (owner, 2026-11-13). A sensor checks what it receives against
corruption and forgery, and it returns an error on anything unexpected (owner,
2026-11-12). Whether the database stores the text, the tree or both is open
(`decisions.md`).

Sampling happens above the evaluation of a rule. A rule that is sampled out
for a record is left out of the set of rules applied to that record, as if the
rule set did not hold it. A record with no rule left is not evaluated at all
(owner, 2026-11-12).

The limits are constants of the engine crate, and no caller sets them (owner,
2026-11-10). The cap on a compiled pattern is the exception, because it is the
default of the `regex` crate (`adr/0041`). The caps on scope tags are in
`adr/0034`.

A rule's identity is its `rule_id` alone, a version 7 UUID that the server
generates (`adr/0020`). An edit is a new rule with a new id (owner,
2026-11-10). The engine never generates an id. It refuses the nil UUID (owner,
2026-11-11) and every UUID version other than 7. The title is a display name in
the `_title` form of `adr/0020`. It is not part of the identity and may
repeat (owner, 2026-11-10). The `Rule` type carries the title, and its JSON
omits the title when it is absent.

The sample rate is a float fraction in `(0, 1]`, and it is 1 when absent
(owner, 2026-11-10). The scope is an ordered map from tag key to tag value
(`adr/0034`), so a rule serializes to the same bytes every time.

When the engine parses a rule document, it refuses a property that the `Rule`
type does not have. This refusal concerns the properties of the rule document.
It does not concern the fields of the records that a rule reads. A rule has no
schema version field.

Validation stops at the first refusal and returns it, as the parser does
(`adr/0042`) (owner, 2026-11-17). It runs its checks in the order in which the
JSON form writes the fields, and inside the expression it checks a node before
its children, so a rule with several defects gets the same refusal every time.
The refusal names its place in the rule document as a JSON pointer, also
inside the expression, as in `/expression/and/1` (owner, 2026-11-16).

Validation holds an expression to 32 nodes from its top to a leaf, with both
counted (owner, 2026-11-16). It prints the expression and refuses a rule whose
printed text is longer than 4096 characters, the cap that the parser holds a
text to, so a rule that validates can be printed and parsed again (owner,
2026-11-16). Validation checks the depth of a tree before anything prints it,
because the printer calls itself once per level and would overflow its stack on
a deep tree (owner, 2026-11-17). Validation does not test whether the parser
could have built a tree, such as whether an `and` has two operands (owner,
2026-11-16).

## Rejected alternatives

The name `fail` for the action that raises, because `throw` says what it
does.

A rule as an OR of groups of conditions, because it cannot compare a field
with a field or nest AND and OR.

A `kind` property in every node, because it is the longest form and the code
that reads the JSON has to buffer a node to find its kind. Nested arrays,
because the code that reads the JSON of the tree and the code that writes it
would have to be written by hand.

A small core of operators that the parser lowers every form to, because a
printed rule would read differently from what was written.

A field as nested nodes, because the planner would fold them back into paths.

The author's text beside the tree, because something would have to check that
the two say the same.

`and` and `or` as pairs, because a rule of many conditions would nest deeper
than a JSON reader accepts.

Sending the text to a sensor, because a sensor would then parse at every sync
and could meet a text that its version of the parser does not accept. Validation
in an SDK, because an SDK executes only rules that the server returned, and the
server validated each of them before it stored it.

A depth of 60, about the deepest tree that the JSON reader reads (owner,
2026-11-16). A check of the depth in the parser, because the parser's cap on
nesting bounds its own stack and the depth of a tree is no condition of parsing
(owner, 2026-11-16).

A cap for each dimension of a tree: the nodes, the items of a list, the
characters of a text and the steps of a path. It needs four new numbers, and
the printed length alone bounds every dimension. Caps derived from the text
cap, such as at most 4096 nodes, because a valid rule could still print to a
text that the parser refuses.

A check in validation for each shape that the grammar builds, and printing a
tree and parsing it back, because validation does not care about a tree that
the parser cannot build, also when one arrives as JSON (owner, 2026-11-16).

Limits that the caller passes to validation, because a rule's validity would
then differ per install. No limits, because an AI client will abuse an
unbounded system (owner, 2026-11-10).

A lineage version number beside the id, because the id already changes with
every edit and lineage and rollback are not goals. An opaque identity
string, because the event query would have to parse it. Accepting the nil UUID
for the convenience of fixtures, because tests must not lower code quality
(owner, 2026-11-11). Accepting any UUID version, because the format would
admit ids the server never issues.

A `rule_name` under the unique-name rule of `adr/0020`, because the test copy
and the live rule exist at the same time. No title in the `Rule` type of the
engine, because every SDK and the event viewer would then join it back from the
server.

A sample rate of one in N as an integer, because 0.3 cannot be written. A
percentage as an integer, because a rate under one percent cannot be
written.

`observe` as the only action until a sensor SDK exists, because adding a variant
later changes the format after the API shipped.

Accepting and ignoring unknown properties, because a misspelled property
would silently become the default.

A schema version field on a rule, because nothing is kept backwards
compatible (`principles.md`, 8).

Every refusal of a rule in one answer, because validation answers like the
parser, which reports its first error (owner, 2026-11-17). A range of bytes in
the text as the place of a refusal inside the expression, because validation
reports the refusal and validation sees the tree. A refusal with a free-text
message only, because the UI could not point at the place of the error.

## Consequences

The web UI edits a rule as text. The engine, the server and every SDK read the
same tree. A limit changes by a commit, and the change invalidates the stored
rules that exceed it. The event query shows a title that may repeat, and the
unique-name rule of `adr/0020` does not apply to rules. No code executes `drop`
or `throw` until a sensor SDK exists. An author with two mistakes in a rule sees
them one at a time. A sum of more than 31 terms in a comparison needs
parentheses that group it. A text that the parser accepts can fail validation
for its printed length, because the printer writes spaces that the author left
out. `validate` does not vouch for the shape of a tree that arrives as JSON,
such as an `and` with one operand. The engine component cites this record.
