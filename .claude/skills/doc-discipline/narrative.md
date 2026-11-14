# Comments inside functions

Goal: the reader that the policy defines can change a function safely after
reading its doc comment, its walkthrough when it has one, and its notes. Each
fact sits at the scope it is about, and every comment describes the code as
it is today.

The rules are "Code comments" and "Prose" in the root `AGENTS.md`. This file
says how to apply them and shows examples. The examples are illustrations of
shape. Do not copy their wording or their semantics.

## Which level a function gets

Each function gets one of three levels. The policy's "Code comments" section
says when a function qualifies for each. A function that forwards, passes
settings through or maps fields one to one is bare, whatever else is true of
it. A constant in it still says where it came from. Test the other functions
against the levels in this order, and the first that applies wins.

1. **Walkthrough.** The reader cannot see the function's plan at any single
   line. The function gets a doc comment, a walkthrough at the top of its
   body, one back-reference per numbered step, and a note at each line where
   the reader stops.
2. **Notes.** The reader would stop at one or more lines of the function. It
   gets a doc comment and a note at each such line.
3. **Bare.** Any other function. Its body has no comment, except that a
   constant says where it came from. It gets a doc comment only where the
   language's convention for public items wants one.

Length and branch count do not move a function between levels. A function
with five obvious branches is bare, and a function with one dense line gets
one note.

Record the decision for every function in scope as one row of the plan before
changing anything:

| function (file:symbol) | level | lines where the reader stops | evidence for each reason |
| ---------------------- | ----- | ---------------------------- | ------------------------ |

**lines where the reader stops** lists each line that gets a note, with the
question the note answers. **evidence for each reason** lists, for each reason
you intend to write in a walkthrough or a note, where you verified it: a
`file:line`, a test, a doc passage or a commit. A reason without evidence is
not written. The function goes under "Rationale unknown" instead.

## Sizes and the step check

The policy gives the size of each kind of comment. A comment above its size
usually holds a second fact. Split it, and move each fact to the step or the
line it concerns.

The step check is `scripts/check_steps.py` in this skill's directory. Run it
over the source files in scope, with one `--deny` for each pattern the policy
lists:

```
python3 <skill dir>/scripts/check_steps.py [--deny REGEX]... <path>...
python3 <skill dir>/scripts/check_steps.py --markdown <file.md>
```

The second form checks the fenced code blocks of a Markdown file whose fence
names a language. The script prints one line per finding and ends with a line
that counts them. It checks the step numbers, the back-references and the
denied patterns. It does not check whether a back-reference sits at the right
line, whether a function that qualifies has a walkthrough, or whether a
comment is true.

## Examples

### A forwarder

INCORRECT:

```
/// Fills masked values using the default null value.
filled(a) -> array
{
    // Get the null value for this type
    v = null_value_of(a.type)

    // Call filled with the null value
    return filled(a, v)
}
```

CORRECT:

```
filled(a) -> array
{
    return filled(a, null_value_of(a.type))
}
```

Every comment in the first version repeats the line below it.

### An algorithm, in pseudocode

The function decides whether one condition of a rule matches a record. The
reader cannot see from any single line why it loops over candidates or when
the policies decide, so it gets a walkthrough.

CORRECT:

```
/// Whether condition `c` matches record `r`. It never raises on bad data.
/// The policies on `c` decide missing fields and uncoercible values.
condition_matches(c, r) -> bool
{
    // A field path can name several values of one record, through a
    // wildcard over an array. The condition matches when any value
    // satisfies the operator. The policies on `c` decide only when no value
    // can.
    //
    // 1. We resolve the path to its candidate values. When there is none,
    //    the missing-field policy decides.
    // 2. We coerce each candidate to the field type and compare it. Both
    //    run in one loop, so the first match skips the remaining coercions.
    // 3. When no candidate coerced, the coercion policy decides.

    // 1. resolve the candidates
    candidates = resolve(c.path, r)
    // A wildcard over an empty array resolves to nothing, which counts as
    // missing.
    if is_empty(candidates) or all_null(candidates)
    {
        return apply_policy(c.on_missing_field)
    }

    // 2. coerce and compare
    any_coerced = false
    for v in candidates
    {
        // A null or uncoercible candidate is skipped, because the others
        // can still decide the condition.
        if is_null(v) { continue }
        x = coerce(v, c.field_type)
        if is_failure(x) { continue }
        any_coerced = true
        if compare(c.op, x, c.value) { return true }
    }

    // 3. apply the coercion policy
    return any_coerced ? false : apply_policy(c.on_coercion_fail)
}
```

The walkthrough says how the function works and why each step is there, and
each back-reference only names its step. The two facts about single lines,
the empty wildcard and the skipped candidate, sit at those lines as notes
without numbers.

INCORRECT:

```
condition_matches(c, r) -> bool
{
    // 1. resolve the field path to candidate values
    // 2. coerce each candidate to the field type
    // 3. compare, matching when any candidate satisfies the operator

    // 1. A wildcard over an empty array resolves to nothing, which counts
    //    as missing.
    candidates = resolve(c.path, r)
    ...

    // 2. and 3. share one loop, so the first match skips the remaining
    //    coercions.
    any_coerced = false
    for v in candidates
    ...
}
```

The walkthrough lists labels, and the reasons sit in numbered paragraphs in
the body, so the reader learns how the function works only at its end. Steps
2 and 3 run in one loop, so they are one step, and the walkthrough gives them
one item.

### A dense line, in pseudocode

The line joins the operands of a chain such as `a - b - c`. `rest` comes from
a parser elsewhere in the module.

INCORRECT, bare:

```
expr = fold(rest, first, (left, (op, right)) -> binary(op, left, right))
```

The reader does not know what each item of `rest` holds, or in which
direction the chain groups.

INCORRECT, a paraphrase:

```
// Fold `rest` into `first` with a closure that takes apart each pair and
// builds a binary node.
expr = fold(rest, first, (left, (op, right)) -> binary(op, left, right))
```

The note names the fold and the closure, which the reader can already see,
and leaves both questions open.

CORRECT:

```
// Groups the chain from the left, so `a - b - c` reads as `(a - b) - c`.
// Each item of `rest` is an operator and the operand after it.
expr = fold(rest, first, (left, (op, right)) -> binary(op, left, right))
```

The note says what the line achieves for the expression and gives the shape
of the value another function built. The example input shows the grouping
that the fold produces.

### A step that begins inside the step before it, in Rust

```rust
pub fn parse(text: &str) -> Result<Expr, SyntaxError> {
    // Parsing works as follows:
    //
    // 1. We split the text into tokens. `lex` does not fail. A text it
    //    cannot split ends in a `Bad` token, and the grammar reports it only
    //    when it gets there, so errors come in the order of the text.
    // 2. We run the grammar from the first token. It gives a tree, no
    //    match, or a refusal such as `UnknownFunction` for `lenn(name) > 3`.
    // 3. On a tree, we resolve the names. The grammar holds every bare name
    //    as an element, because in `any(test for name in list)` the name
    //    comes after the test that uses it.
    // 4. Otherwise we build the error from the refusal and from the log of
    //    the token tests. A generic "expected" error first gets two
    //    targeted reasons tried, and neither changes which texts are valid.

    // 1. split into tokens
    let input = Input::new(text, lex(text));

    // 2. run the grammar
    let refusal = match rule(&input, 0) {
        Ok(Some((mut tree, _))) => {
            // 3. resolve the names
            resolve(&mut tree, &mut Vec::new());
            return Ok(tree);
        }
        // Without a match, the log is the only source of the error, as for
        // `a > 1 b`.
        Ok(None) => None,
        Err(refusal) => Some(refusal),
    };

    // 4. build the error
    let error = explain(&input, refusal);
    // A refusal or a tokenizer's reason already says what is wrong.
    if !matches!(error.reason, Reason::Expected { .. }) {
        return Err(error);
    }
    // `AND` in `a > 1 AND b < 2`, then `lambda` in `lambda x: x > 1`.
    let targeted = miscased_word(&input).or_else(|| python_only_word(&input));
    Err(targeted.unwrap_or_else(|| with_words(&input, error)))
}
```

Step 3 begins inside the arm of step 2 that holds a tree, so its
back-reference sits at the first line of that arm. Step 2 goes on with the
other two arms, and step 4 begins after the match. The three notes carry no
number, because each explains one line: the arm without a match, the guard
and the targeted reasons.

### A doc comment that explains the implementation, in Go

INCORRECT:

```
// Call runs f on a session authenticated as the user. The breaker gates
// the call. The user's pool leases a session, dialing one on demand, and
// runs f. The binding's IsBadSession and Lease.Done decide from f's error
// whether the pool keeps the session. This layer adds two decisions, both
// per cluster. Only errors that are evidence about the cluster
// (IsClusterUnavailable) feed the breaker. An idempotent read can opt in
// to one retry after a retryable failure, and a refused credential is
// never retried. A retry after a bad session runs on a fresh session,
// because the pool discarded the old one. A retry after any other
// retryable failure may run on the same session.
func (c *Cluster) Call(ctx context.Context, u User, f func(*Session) error, opts ...CallOption) error {
```

CORRECT:

```go
// Call runs f on a session authenticated as u. It returns a
// *BreakerOpenError while the cluster is unreachable. WithReadRetry allows
// one retry of an idempotent read.
func (c *Cluster) Call(ctx context.Context, u User, f func(*Session) error, opts ...CallOption) error {
	...
		// The caller's own cancellation says nothing about the cluster.
		if callerLeft(err) {
			return err
		}
		c.feedBreaker(err)
		// A second dial with refused credentials is refused the same way.
		if !cc.retry || !qdbapi.IsRetryable(err) || errors.Is(err, ErrCredentialsRefused) {
```

The first version tells callers how the pool and the breaker work, and each
change to `Call` added a clause to it. The second version gives callers the
contract and puts each reason at the line it explains. The facts about the
pool belong to the pool's own comments.

### A change, in Rust

The code before the change:

```rust
/// Readiness: the instance may receive traffic. `ok` as soon as the
/// listener is up.
async fn readyz() -> &'static str {
    // TODO(store): probe the store.
    "ok"
}
```

The change adds a store probe. INCORRECT, the old comment patched:

```
/// Readiness: the instance may receive traffic. `ok` as soon as the
/// listener is up, and now also once the store answers.
async fn readyz(State(s): State<Store>) -> StatusCode {
    // The store probe replaces the earlier unconditional `ok`.
    s.ping().await.map_or(StatusCode::SERVICE_UNAVAILABLE, |()| StatusCode::OK)
}
```

CORRECT, the comment written from the new code:

```rust
/// Readiness: the instance may receive traffic. 200 once the store answers
/// a ping, 503 while it does not.
async fn readyz(State(s): State<Store>) -> StatusCode {
    s.ping().await.map_or(StatusCode::SERVICE_UNAVAILABLE, |()| StatusCode::OK)
}
```

The patched version keeps the sentence about the listener, adds "now", and
explains the history in the body. The rewritten version reads as if the
function had always probed the store.

## Applying it to existing code

1. Decide the level with the ordered rules above. A bare function loses its
   noise comments, and nothing else changes. Commented-out code is reported
   under "Needs a human" and stays in place.
2. Read the whole function and what its callers expect before writing.
3. Write the function's comments from the code, before you reread the
   existing ones. Then compare with the existing comments and carry over
   only what the code cannot show: a verified reason, a `TODO(name)` or
   `XXX(name)` tag, a name a reader searches for. Existing wording does not
   survive because it is there. A comment that reads as a patch on an
   earlier version is rewritten whole, even when every clause is true.
4. When the why cannot be verified, write the what and ask the owner for
   the reason. An audit lists the function under "Rationale unknown" as a
   question. Leave performance motives and history to the owner's answer.
5. When the walkthrough needs more than about seven steps, the function is a
   split candidate. Comment it as it is and list it under "Needs a human".
   A standard algorithm, and a body that tests a chain of independent cases,
   get a walkthrough without numbers, as the policy says.
6. Read the comments back in three passes, in this order:
   1. **Truth.** Read each comment as a separate claim and check it against
      the code directly below it. Check each walkthrough item against the
      code from its back-reference to the next one. Check each example input
      by tracing it through the code or running it in a test, and delete an
      example you cannot check. Run the step check and fix each finding.
   2. **Cut.** Delete or rewrite each of these comments:
      - a note that repeats what the line below it says
      - a note that restates a walkthrough item
      - a walkthrough sentence about one line, which moves to that line as a
        note
      - a note that explains what a language feature does in general
      - a byte offset where the code computes no range
      - a number outside a walkthrough and its back-references
      - a comment that mentions an earlier version, a plan, a review, an
        issue or the conversation
      - a mark of who decided something or when, which belongs in the
        decision record
      - a citation of a decision record in a form other than the one the
        policy names
      - an implementation detail in a doc comment
      - a rejected alternative that a reader of that line would not propose
      - a sentence that reads like one of the "Written" sentences in the
        policy's prose rules
   3. **Gaps.** The session that wrote the code does not judge its own
      gaps, because it already knows what each line does. It runs
      `/doc-read` as `checks.md` says and closes each stop the reader
      lists. An audit lists each line where the reader would stop, and each
      function that qualifies for a walkthrough and has none, as a question
      under "Rationale unknown".
7. New code elsewhere in this project follows the same rules from the start.
   "Repairing existing code and documents" in `SKILL.md` is the repair path.
