# The checks after a change

The session that owns a change runs these checks after its last code commit
and before the change leaves the session. Each check is a script's output or
another agent's answer, and the session saves each answer where its workflow
says. `<base>` is the revision the change started from.

## The steps

1. Run the step check over what the change touched, with one `--deny` for
   each pattern the policy lists, and fix and commit each finding:

   ```
   python3 <skill dir>/scripts/check_steps.py [--deny REGEX]... \
     --git-range <base>...HEAD --markdown-too
   ```

2. Run `scripts/change_parts.py` in this skill's directory with `python3` over
   `<base>...HEAD`. It prints the parts of the change, about 300 changed lines
   each. Then call `/doc-read <base>...HEAD <part>` once for each part, with the
   range and the part number and no other word. The calls may run side by
   side. Save the answers verbatim in the order of the parts. A reader that
   gets a large change whole spreads its attention thin and reports little
   toward its end.
3. Note the head with `git rev-parse HEAD` before you close the first stop.
   Close every stop of the answers. Test the four ways in this order, and use
   the first that answers the question:
   1. a rename or a restructuring that answers the question
   2. a comment written from evidence you can name
   3. a question to the owner
   4. a decline with its reason, when the question asks what the language
      or its standard library does, or when the line already answers it

   Rewrite or delete a comment that the reader said told it nothing. Check a
   statement that the reader believed false against the code, and correct
   the statement or the code.

4. When step 3 changed a comment, a document or the code, run a second
   round. In it, list the parts of `<head>...HEAD` with the head you noted in
   step 3, and call `/doc-read <head>...HEAD <part>` for each part as in step 2,
   so that the new readers read what step 3 changed. Save their answers and
   close their stops as in step 3. No third round follows.
5. Run `/doc-audit <base>...HEAD plan: <plan>` when the change has a plan,
   and `/doc-audit <base>...HEAD` when it has none. Save its report
   verbatim. Apply each row, or decline it with its reason.
6. Run the step check again, fix each finding, and repeat until its last line
   reports 0 findings.

A question to the owner from steps 3 to 5 is asked before the commit that
needs the answer. A comment that waits for an answer is written after it
arrives.

## What the reviewer sees

The session shows its reviewer the last line of each step check, every stop
with its closure, every decline with its reason, and the rows of the audit
as applied or declined. Each stop gets one of these lines:

```
<path>:<line> | rename <sha>
<path>:<line> | comment <sha>
<path>:<line> | question
<path>:<line> | decline: <reason>
```

A check counts as run only when its saved answer exists. A session that
reports a check without the saved answer reports a check that did not run.

## Examples

Correct: the session lists two parts of `main...HEAD`, runs
`/doc-read main...HEAD 1` and `/doc-read main...HEAD 2`, saves the answers
with twelve stops, renames one variable, writes nine notes from the code and
the tests, asks the owner about a constant, and declines one stop because the
standard library defines the term. The reader of the second round finds one
note that told it nothing, and the session rewrites it. The audit proposes
four rows, and the session applies three and declines one with its reason.

Incorrect: the session reads its own comments again and reports that it
found no gaps. It already knows what each line does, so it cannot see where
a reader stops. Or the session adds a hint to the call, such as
`/doc-read main...HEAD 1 the parser is the risky part`. A hint tells the
reader what the author knows, and `/doc-read` passes the range and the part
number alone to the reader.
Or the session asks a subagent to write the missing comments. The subagent
does not know the reasons, so it writes paraphrases and guesses.
