# 0005. One vocabulary for failure behavior

Date: 2026-11-28.

## Context

A sensor, a rule and the operations of a rule each meet something that goes
wrong, such as an unreachable server, a missing field or an operation that
cannot be done. One vocabulary names what each of them does then.

A rule meets records that lack a field it reads, and records on which an
operation cannot be done. Missing keys are ordinary in pipeline data.

SQL's three-valued logic says what an expression does with a value that is
not there, and Arrow and Spark implement that logic.

## Decision

Each sensor is set to one of three behaviors for when the server is unreachable.
`pass-through`, the default, turns the rules off, lets the records flow and
prints a warning at most. `cached` keeps using the last rules the sensor synced.
`halt` raises an error into the pipeline, and a sensor opts into it, because
TrapperKeeper being down should not affect a production pipeline by default
(owner, 2026-11-15).

A rule treats a value that is not there as a null in the data. That value is no
fault and does not turn the rule off. A missing key is a null (owner,
2026-11-14). A NaN is a null, whatever the source of the records (owner,
2026-11-26). The source says how it marks a value that is not there, such as
by a bitmap or by a byte mask (`adr/0036`). An index beyond the end of a list, a
list that is absent under `any` or `all`, and a null element of a list are
nulls too (owner, 2026-11-14).

A rule applies to a record that holds a null. The sensor does not report that
null. When no record has a field that a rule reads, the sensor logs no warning
and sends no report (owner, 2026-11-14).

A null follows SQL's three-valued logic. The places where a rule differs from
SQL are listed below (owner, 2026-11-15).

An arithmetic operation, a function or a method on a null gives a null, so
`x + 1` is a null when `x` is. A comparison on a null gives unknown, which is
the null of a boolean. So `x + 1 > 5` and `x > 1` are both unknown when `x`
is a null (owner, 2026-11-14).

`not` keeps unknown. `and` gives false when an operand is false. Otherwise it
gives unknown when an operand is unknown. `or` gives true when an operand is
true. Otherwise it gives unknown when an operand is unknown. The order of the
operands does not count for a null (owner, 2026-11-14).

`any` gives true when the test is true for some element. Otherwise it gives
unknown when the test is unknown for some element or when the list is absent.
`all` gives false when the test is false for some element. Otherwise it gives
unknown when the test is unknown for some element or when the list is absent.

A conditional whose test is unknown gives unknown. A fault in either of its
branches then does not count (owner, 2026-11-14). A null where a condition is
expected is unknown.

A record matches a rule only when the expression gives true.

Five tests are never unknown: `is None`, `is not None`, `x == None`,
`x != None` and `isinstance`. `isinstance` gives false for a null.

`x == None` means `x is None`, and `x != None` means `x is not None`. A
`None` in a membership list matches a null, so `x in ("paid", None)` means
`x == "paid" or x == None` (owner, 2026-11-14). With the literal on the left
of `in`, as in `None in tags`, the test is true when the list `tags` holds a
null element (owner, 2026-11-16). A list search follows `any` over the
equalities of the elements, so a needle that no element can equal gives
false, or unknown where an element is a null (owner, 2026-11-28). Only the
literal `None` has this meaning.
`a == b` on two fields that both hold a null gives unknown.

The meaning of `in` follows the type of its right side. When the rule writes
the list, as in `x in [1, 2]`, `in` tests membership. When the right side is a
field that holds a text, `in` searches the text for a substring, and when the
field holds a list, `in` searches its elements (owner, 2026-11-28).

Validation (`adr/0033`) refuses the literal `None` beside an ordering or an
arithmetic operator, under a minus, and after `in` and `not in`, because such a
rule could never match. A field that holds a null there is data, so `x + 1` is a
null and `x < y` is unknown (owner, 2026-11-14). A search `x in y`, where `y`
is a field, gives unknown when `y` holds a null (owner, 2026-11-16). A list
search also gives unknown when no element equals the needle and an element is
a null, as `any` does.

A rule differs from SQL in three places. The first is the literal `None`.
Under `==`, under `!=`, in a membership list and on the left of `in` it tests
for a null, and beside an ordering or an arithmetic operator validation
refuses it. SQL gives unknown in each of these places. The second is a
conditional with an unknown test, which gives unknown, where SQL's `CASE`
takes the else branch. The third is the fault. This record says which
operations are faults and what a fault does. SQL engines have their own
answers, and they differ on a division by zero.

A fault is an operation that cannot be done on values that are there (owner,
2026-11-13, 2026-11-27):

- an ordering compares a value that is not a number, such as a text or a
  boolean
- a division by zero
- an integer overflow
- a value that is no boolean where a condition is expected
- `in` with a value that it cannot search, such as `5 in "abc"`

`==` compares values of any two types, as in Python, and a text never equals
an int. So `x == 5` is
false and `x != 5` true where `x` holds a text (owner, 2026-11-28). An int and
a float can be equal, and a boolean never equals a number (`adr/0041`).

A rule that meets a fault on a record does not apply to that record. The rule
is off for that record. The rule behaves as if it did not exist for that
record. The rule never raises into the pipeline (owner, 2026-11-11). The SDK
logs a warning and sends a report upstream. The SDK limits the rate of the
same warning (owner, 2026-11-13).

`and`, `or` and the conditional follow written order for faults, as in Python.
When the left operand of `and` is false, or the left operand of `or` is true,
the result is decided. The right operand then does not count for the result, and
a fault in it does not happen. That is why the guard in `y != 0 and x / y > 5`
protects the division. When the left operand is unknown, the right one is
evaluated and its fault counts. In a conditional the test comes first, so a
fault in the test counts. A fault in the branch that the test does not pick does
not happen (owner, 2026-11-13, 2026-11-14).

Under `any` and `all`, an element with a fault turns the rule off for that
record, whatever the other elements say. The order of the elements does not
count (owner, 2026-11-13).

No part of a rule's expression carries a setting for a missing field or for a
value of the wrong type (owner, 2026-11-13).

## Rejected alternatives

A setting in a rule's expression for a missing field and for a value of the
wrong type, because a rule should say in its own text what it does.

Semantics of our own for a null, because every reader and every host engine
already knows SQL's.

A NaN that a source treats as a value, compared as IEEE 754 defines, because
the same numbers would then give different outcomes through two sources.

A missing value as a fault, because rules would then be off for ordinary
records. Reports would flow for ordinary records too. And `amount > 500 or
status == "paid"` would give another outcome than the same rule with its
operands swapped, because written order decides which fault counts.

Plain false for a comparison with a null, because `not (amount > 500)` would
match every record without an amount and `amount <= 500` would not.

An index beyond the end of a list, an absent list and a null element as
faults, because ragged and absent lists are as common as missing keys. An
absent list as an empty list, because `all(...)` would match every record
without the list.

A fault as unknown that is reported, because every guarded division would
report. A fault that counts wherever it stands, because no guard could
protect a division.

One report for a field that no record had, because a typo shows as a rule that
never matches, and `principles.md`, 2 leaves its detection to event-informed
tooling on the server (owner, 2026-11-14). A report per record under a rate
limit, because ordinary records with optional keys would keep producing reports.

The else branch for a conditional whose test is unknown, as in SQL's `CASE`,
because `("high" if amount > 500 else "low") == "low"` would match every
record without an amount.

Refusing `x == None` with a pointer to `x is None`. The owner chose Python's
meaning, so that a Python author can write what they know (owner,
2026-11-14).

SQL's meaning for `x == None`, because the rule would parse and never match.

Unknown for a `None` in a membership list, as in SQL, because that `None`
would never match.

An equality in which two nulls are equal, because `a == b` would then match
every record that has neither field.

A fault for `==` between values of types that cannot be equal, because the
order of a membership list would then decide the result. `x in ["a", 1]`
would fault on every int record, and `x in [1, "a"]` would match the records
that hold 1.

`isinstance` on a null as unknown, because `not isinstance(x, int)` is the
way to find the records whose value has another type or is absent.

A fault for a null where a condition is expected, because `active and
amount > 5` would turn off on a record without `active`.

Unknown for the literal `None` beside an ordering or an arithmetic operator,
because the rule would never match and nothing would report it. A fault there,
because every record would be reported.

Turning the rule off for a whole batch when one record meets a fault, because
a record's outcome must not depend on its batch.

An element under `any` or `all` that decides the result and so hides an
element with a fault, because bad data in a list would then go unreported. A
true element decides `any`, for example.

## Consequences

A rule over an optional field needs no guard. A guard still protects against
a fault. A typo in a field path matches nothing, and nothing reports it.
`x == None` and `x == y` give different results when `y` holds a null.

Unknown needs no mask of its own. A column is an array of values with a mask
whose set bits say which values are there (`adr/0036`), and a boolean result
has the same form. Unknown is a boolean whose mask bit is not set. The mask
of a comparison is the AND of the masks of its operands. `not` flips the
values and keeps the mask. `and` and `or` combine two such results with a few
bitwise operations per word. A record matches where the value and the mask
are both set. A float column always gets a mask, from the pass that finds its
NaNs. Any other column without a mask gives a result without one.

Every float column costs one pass that finds its NaNs. A NaN that arithmetic
makes, such as infinity minus infinity, becomes a null, and a source that keeps
a NaN as a value loses that distinction (owner, 2026-11-26).

The evaluator records the faults of an operation as one bit per row, in a
bitmap of their own. Only an operation that can fault, such as a division, has
such a bitmap.

The evaluator must meet three conditions, so that it can evaluate operands in
written order for faults in a loop with no branch per row. First, an operation
on a null is never a fault, so a fault counts only where its operands are there.
Second, no loop runs an instruction that traps, as an integer division by zero
does. A division therefore replaces a divisor by 1 when it is zero or when its
mask bit is not set. It marks a fault for both, and the first condition drops
the fault where the mask bit is not set. Third, a skipped operand may still be
computed, and its faults are dropped. These three conditions follow from
`adr/0035` and `adr/0036`, and no measurement supports them. They are measured
when the evaluator exists.

`any` and `all` reduce the results of their test twice per record. One reduction
finds an element that decides, and the other finds an element whose test is
unknown. On Spark a rule is translated into a Spark expression (`adr/0040`), and
the translation of a conditional needs one more `WHEN test IS NULL` arm. The
conformance fixtures must hold one fixture for each difference from SQL
(`adr/0006`).

The literal `None` parses and validates as an argument of a function and as
the receiver of a method, as in `len(None)` and `None.upper()`. Such a rule
gives unknown for every record and matches nothing in silence. A check of
types, which is to be built together with the planner of `adr/0035`, would
refuse these forms (owner, 2026-11-16).

An adapter puts the rows whose value has a type other than the column's into
a second column (`adr/0036`). A rule gives each such record what its
operator gives for the record's own type. So `x == "a"` is false for a record
that holds an int, and `x > 5` is a fault for a record that holds a text.

The fixtures and every SDK must use the vocabulary of this record and cite it:
`pass-through`, `halt`, `cached`, null, unknown, fault and off.
