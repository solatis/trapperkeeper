# 0005. One vocabulary for failure behavior

Date: 2026-11-13.

## Context

The retired first attempt had three vocabularies for what happens when
something goes wrong.

A rule meets records it cannot be evaluated on. A field is missing, or a
value has a type the rule cannot use.

## Decision

Per sensor, when the server is unreachable: `pass-through` (default), `halt`,
`cached`.

A rule that cannot be evaluated on a record does not apply to that record. It
behaves as if it did not exist for that record, and it never raises into the
pipeline (owner, 2026-11-11). It is reported. The SDK logs a warning and sends
a report upstream, and it limits the rate of the same warning (owner,
2026-11-13).

A rule cannot be evaluated on a record in these cases (owner, 2026-11-13):

- a field that an operator, a function or a method uses is missing
- a value is null, which counts as missing
- two values have incompatible types
- an ordering compares text
- a division by zero
- an integer overflow
- an index beyond the end of a list
- a list that is absent under `any` or `all`
- a value that is no boolean where a condition is expected

`and`, `or` and the conditional expression follow written order, so a guard
such as `x is not None and x > 5` works. Three things never turn a rule off:
`is None` and `is not None`, `isinstance`, and whatever written order skips
(owner, 2026-11-13).

In a list, an element that cannot be evaluated turns the rule off for that
record, whatever the other elements say. The order of the elements does not
count (owner, 2026-11-13).

A condition carries no settings (owner, 2026-11-13).

## Rejected alternatives

The three vocabularies of the first attempt.

A setting per condition for a missing field and for a wrong type, because a
rule should say in its own text what it does.

A missing field that simply does not match, because a typo in a field path
would stay invisible.

A failure that counts only when it decides the result, as NULL does in SQL,
because a misconfigured branch could hide.

Turning the rule off for a whole batch when one record cannot be evaluated,
because a record's outcome must not depend on its batch.

In a list, an element that decides and hides a bad element, because bad data
in a list would then go unreported.

## Consequences

A rule over an optional field needs a guard. Written order matters, as in
Python. A typo in a field path shows up as a report.

For dicts and maps an adapter keeps the rows whose value had the wrong type
(`adr/0036`).

The fixtures and every SDK use these words. They cite this record.
