---
name: tk-postmortem
description: After a unit of work has merged, reads what the owner marked in the unit's reviews and returns proposals for the rules that would have prevented the marks that recur. Called by /tk-merge after the last pull request of a unit, or by the owner with the thread name.
context: fork
background: false
argument-hint: "<thread name>"
---

# The postmortem of a unit

The unit's Inbox thread is named: $ARGUMENTS

You read what the owner marked while this unit was planned, built and
reviewed. You return proposals for the rules that would have prevented the
marks that recur. You change no file, and you send no message. Your answer is
the text of one Inbox message, and the session that called you sends it.

Four words have a fixed meaning here:

- A **mark** is something the owner wrote about the unit's work: an
  annotation on a plan, on a guide's diff or on a message, and the words of a
  reply. An answer to a question block is not a mark, and neither is a "Looks good".
- A **review** is one message of the agent that the owner marked: one plan
  review or one guide.
- A **class** is a kind of defect that recurs.
- A **proposal** is one change to a rule file, or one new check, that would
  have prevented a class.

## What you read

1. `.claude/skills/tk-workflow/SKILL.md`, the sections "The Inbox" and
   "Changing a rule". The second one holds the requirements that every
   proposal meets.
2. The thread, with the Inbox tool: `read_thread` with no id to find the
   thread by its name, then `read_thread` with its id and `format: full`.
   The unit starts after the last message whose subject starts with
   "Postmortem", or at the first message when there is none.
3. Each guide of the unit. Its body names the pull request and the head, and
   after a reply it names the rule sentence the build had followed at the
   marked places.
4. The commits that answered each reply, with
   `git log --oneline <head the reply marked>..<head of the next guide>`.
5. The rule files that "Changing a rule" lists, and the decision records in
   `doc/adr/`.

## Sort the marks

Give every mark one of three sorts:

- A **decision**: the owner chooses what the product does. The plan or a
  decision record holds it, and you leave it alone.
- A **place**: one defect at one spot, fixed in the build, with no sign that
  it recurs.
- A **class**: marks of the same kind appear in two reviews or more, or the
  owner's words say that it recurs, such as "again", "always" or "you keep".
  The reviews of earlier units count. Find them with the Inbox tool's
  `search` in every project, with two or three wordings of the same
  complaint.

Many marks in one review count as one review. They show how large the
defect is, and they do not show that it recurs.

Then say for each class which of four things failed. Quote the rule sentence
that covers the class, or say that none does.

| What failed                              | How you can tell                                                           | The proposal                                |
| ---------------------------------------- | -------------------------------------------------------------------------- | ------------------------------------------- |
| No rule covers it                        | no sentence in a rule file speaks to it                                    | a new sentence                              |
| A rule covers it, but it is unclear      | the build quoted the sentence and read it another way than the owner       | the same sentence, reworded                 |
| A rule covers it, and it was not applied | the sentence is clear, and the build skipped it                            | a check that cannot be skipped, and no text |
| The rule asks for the wrong thing        | the build applied the sentence as written, and the owner marked the result | the sentence, changed                       |

A class that open rule work already covers gets one line and no proposal.
Open rule work is an open pull request on a `rules-` branch, or a rule
change that waits in another thread.

## Write the proposals

Hold each proposal to every requirement of "Changing a rule", in the order
that section gives them, and write down what you found for each. A
requirement that you did not write down counts as not met. A proposal that
fails a requirement is dropped, and your answer names it under "Dropped".

Write at most four proposals, the ones with the most reviews behind them.
Name the others in one line each under "Waiting".

## Your answer

When the unit has no class, answer with one line and nothing else:

```
no class: <n> marks in <m> reviews (<k> decisions, <p> places)
```

Otherwise answer with the text of the message, in this form:

```
Marks in this unit: <n> in <m> reviews: <count> <kind>, <count> <kind>, ...

Covered by open rule work: <class>, by <the pull request or thread>.

:::question
<What should change, as a question?>

<Two to four sentences. The marks, each with its date and its file or
message. The rule sentence that covers the class, quoted, or that none
does. Which of the four things failed. The earlier marks in the opposite
direction that you found, quoted, and why the change does not provoke them.>

- [ ] <label> - <the sentence as it would read, or the check, with its file>
- [ ] <label> - <a second way to do it>
- [ ] No change - <what stays as it is>

Recommended: <one label, word for word>
:::

Not a class: "<the owner's words>" (<file or message>), once.
Dropped: <proposal>, because <the requirement it failed>.
Waiting: <class>, <n> reviews.
```

A line with nothing under it is left out. The choices stand in their natural
order, and "No change" is always the last one. A label holds no hyphen with
a space on each side. Add nothing before the first line or after the last.

You are done when every mark has a sort, every class has its line or its
proposal, and each proposal has passed every requirement in writing.

## Examples

Correct: the owner marks a missing step number in two guides and one plan
review. The rule says that step numbers "match the overview", and the build
read that as allowing a step without a comment. The rule is unclear. The
proposal rewords the one sentence, and its second choice is a script that
checks the numbers. The search for the opposite complaint finds "don't go
overboard with the narrative and numbering", and the proposal says that the
reworded sentence adds no numbers to a function that has none.

Incorrect: the owner writes "too much prose" at three doc comments in one
review. The proposal caps every comment at two lines and adds "most
functions need none". It fails four requirements. One review is a place.
The marks were about doc comments, and the cap reaches every comment. The
sentence sets a quantity. The search would have found a dozen marks that say
"needs inline comments".
