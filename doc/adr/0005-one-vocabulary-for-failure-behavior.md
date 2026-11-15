# 0005. One vocabulary for failure behavior

Date: 2026-11-14.

## Context

The retired first attempt had three vocabularies for what happens when
something goes wrong.

A rule meets records that lack a field it reads, and records on which an
operation cannot be done. Missing keys are ordinary in pipeline data.

SQL has settled what an expression does with a value that is not there. Its
three-valued logic is the answer, and Arrow and Spark implement it.

## Decision

Per sensor, when the server is unreachable: `pass-through` (default), `halt`,
`cached`.

A value that is not there is data. A missing key is a null (owner,
2026-11-14). The source of the records, such as pandas or Arrow, says what
else is "no value" (`adr/0036`). An index beyond the end of a list, a list
that is absent under `any` or `all`, and a null element of a list are nulls
too (owner, 2026-11-14).

A rule still applies to a record that holds a null, and the null is not
reported. The sensor does nothing about a field that no record has (owner,
2026-11-14).

A null follows SQL's three-valued logic. The places where a rule differs from
SQL are listed below (owner, 2026-11-15).

An arithmetic operation, a function or a method on a null gives a null, so
`x + 1` is a null when `x` is. A comparison on a null gives unknown, which is
the null of a boolean. So `x + 1 > 5` and `x > 1` are both unknown when `x`
is a null (owner, 2026-11-14).

`not` keeps unknown. `and` gives false when an operand is false, and
otherwise unknown when an operand is unknown. `or` gives true when an operand
is true, and otherwise unknown when an operand is unknown. The order of the
operands does not count for a null (owner, 2026-11-14).

`any` gives true when the test is true for some element. Otherwise it gives
unknown when the test is unknown for some element or when the list is absent.
`all` gives false when the test is false for some element, and otherwise
unknown in the same two cases.

A conditional whose test is unknown gives unknown, and a fault in either
branch does not count (owner, 2026-11-14). A null where a condition is
expected is unknown.

A record matches a rule only when the expression gives true.

Five tests are never unknown: `is None`, `is not None`, `x == None`,
`x != None` and `isinstance`. `isinstance` gives false for a null.

`x == None` means `x is None`, and `x != None` means `x is not None`. A
`None` in a membership list matches a null, so `x in ("paid", None)` means
`x == "paid" or x == None` (owner, 2026-11-14). Only the literal `None` has
this meaning. `a == b` on two fields that both hold a null gives unknown.

The parser refuses the literal `None` beside an ordering or an arithmetic
operator, because such a rule could never match. A field that holds a null
there is data, so `x + 1` is a null and `x < y` is unknown (owner,
2026-11-14).

A rule differs from SQL in three places. The first is the literal `None`.
Under `==`, under `!=` and in a membership list it tests for a null, and
beside an ordering or an arithmetic operator the parser refuses it. SQL gives
unknown in each of these places. The second is a conditional with an unknown
test, which gives unknown, where SQL's `CASE` takes the else branch. The
third is the fault. A fault turns the rule off for the record, and SQL
engines differ on what a division by zero does.

A fault is an operation that cannot be done on values that are there (owner,
2026-11-13):

- two values have incompatible types
- an ordering compares text
- a division by zero
- an integer overflow
- a value that is no boolean where a condition is expected

A rule that meets a fault on a record does not apply to that record. It
behaves as if it did not exist for that record, and it never raises into the
pipeline (owner, 2026-11-11). It is reported. The SDK logs a warning and sends
a report upstream, and it limits the rate of the same warning (owner,
2026-11-13).

`and`, `or` and the conditional follow written order for faults, as in
Python. When the left operand of `and` is false, or the left operand of `or`
is true, the result is decided. The right operand is then skipped, and a
fault in it does not happen, so the guard of `y != 0 and x / y > 5` works.
When the left operand is unknown, the right one is evaluated and its fault
counts. In a conditional the test comes first, and a fault in the branch that
the test does not pick does not happen (owner, 2026-11-13, 2026-11-14).

Under `any` and `all`, an element with a fault turns the rule off for that
record, whatever the other elements say. The order of the elements does not
count (owner, 2026-11-13).

A condition carries no settings (owner, 2026-11-13).

## Rejected alternatives

The three vocabularies of the first attempt.

A setting per condition for a missing field and for a wrong type, because a
rule should say in its own text what it does.

Semantics of our own for a null, because every reader and every host engine
already knows SQL's.

A missing value as a fault. Rules went dark and reports flowed for ordinary
records, and `amount > 500 or status == "paid"` gave another outcome than the
same rule with its operands the other way round.

Plain false for a comparison with a null, because `not (amount > 500)` would
match every record without an amount and `amount <= 500` would not.

An index beyond the end of a list, an absent list and a null element as
faults, because ragged and absent lists are as common as missing keys. An
absent list as an empty list, because `all(...)` would match every record
without the list.

A fault as unknown that is reported, because every guarded division would
report. A fault that counts wherever it stands, because no guard could
protect a division.

One report for a field that no record had. The owner chose no report (owner,
2026-11-14). A report per record under a rate limit, because ordinary records
with optional keys keep producing reports.

The else branch for a conditional whose test is unknown, as in SQL's `CASE`,
because `("high" if amount > 500 else "low") == "low"` would match every
record without an amount.

Refusing `x == None` with a pointer to `x is None`. The owner chose Python's
meaning, so that a Python author can write what they know (owner,
2026-11-14). SQL's meaning for `x == None`, because the rule would parse and
never match.

Unknown for a `None` in a membership list, as in SQL, because that `None`
would never match.

An equality in which two nulls are equal, because `a == b` would then match
every record that has neither field.

`isinstance` on a null as unknown, because `not isinstance(x, int)` is the
way to find the records whose value has another type or is absent.

A fault for a null where a condition is expected, because `active and
amount > 5` would turn off on a record without `active`.

Unknown for the literal `None` beside an ordering or an arithmetic operator,
because the rule would be dead in silence. A fault there, because every
record would be reported.

Turning the rule off for a whole batch when one record meets a fault, because
a record's outcome must not depend on its batch.

Under `any` and `all`, an element that decides and hides an element with a
fault, because bad data in a list would then go unreported.

## Consequences

A rule over an optional field needs no guard, and a guard still protects
against a fault. A typo in a field path matches nothing, and nothing reports
it. The literal `None` and a field that holds a null differ under `==`, so
`x == None` and `x == y` differ when `y` holds a null.

Unknown needs no mask of its own. A column is an array of values with a mask
whose set bits say which values are there (`adr/0036`), and a boolean result
has the same form. Unknown is a boolean whose mask bit is not set. A
comparison passes the mask of its operands on. `not` flips the values and
keeps the mask. `and` and `or` combine two such results with a few bitwise
operations per word. A record matches where the value and the mask are both
set. A column without a mask gives a result without one.

The evaluator must keep three conditions, so that written order for faults
stays a loop without a branch per row. First, an operation on a null is never
a fault, so a fault counts only where its operands are there. Second, no loop
runs an instruction that traps, as an integer division by zero does. A
division therefore replaces a divisor that is zero or under a null by 1 and
marks a fault, and the first condition drops that fault for the null. Third,
a skipped operand may still be computed, and its faults are dropped. These
three are reasoned from `adr/0035` and `adr/0036`, and no code ran. They are
measured when the evaluator exists.

`any` and `all` reduce the results of their test twice per record. One
reduction finds an element that decides, and the other finds an element whose
test is unknown. A translation to Spark needs one more `WHEN test IS NULL`
arm for a conditional (`adr/0040`). Each difference from SQL is held by a
fixture (`adr/0006`).

A column has one type (`adr/0036`). For dicts and maps an adapter keeps the
rows whose value had another type in a list beside the column, so that the
rule is off for those records alone.

The fixtures and every SDK use these words. They cite this record.
