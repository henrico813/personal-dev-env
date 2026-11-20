# What waits on the user

Read this before you ask the user a question, in an update or on its
own, ask for an approval, or list pull requests to review.

## Contents

- Ask in the short form
- Approvals, deadlines and reviews

## Ask in the short form

The user answers on a phone. Every question to the user, from any
agent, including a hub that runs other sessions, takes this form, in
an update or on its own:

```markdown
**Q7. Lessons rework: how the repeat search survives squash merges**
- A: the merge step copies the fix lines into the squash message **(rec)**
- B: merge fix PRs by rebase
- C: search merged PRs on GitHub

**Q8. Model routing: do you want a routing table and one start command?**
- 1: yes, redesign around them **(rec)**
- 2: partly, plus something else
- 3: no

Reply like `Q7A Q8-1`.
```

- Give each question a bold title that starts with its number, such
  as `**Q7. `. The title names the work in plain words first, then
  what the user decides. A card key, if any, comes after those words
  in parentheses, never first.
- Number each question `Q` and the next number not yet used in this
  session, counting every message, such as `Q7` after `Q6`. A
  question asked again keeps its number, and no number names a second
  question, since the user may answer an older message after a newer
  one arrives. The `Q` sets question numbers apart from numbered
  actions. After a handoff, keep the numbers it gives and number new
  ones above the highest; after compaction, above the highest you see.
- Put the choices right under the title, with no blank line. Write
  each as `- A: ` and one short line, at most 72 characters. The
  count includes `- A: ` and `(rec)`, but not bold and code marks.
  Give 2 or 3 choices.
- For an open question, such as a name to pick, give your best one or
  two answers and a last choice such as `C: another, which you name`.
- Key choices with letters. Use numbers, `- 1: `, when the choices
  are steps on a scale, such as yes, partly and no.
- Mark the choice you recommend with `**(rec)**`, on one choice only.
  When you ask which reading of the user's message they meant, mark
  the likelier one.
- End with one line that starts with `Reply like`, for all the
  questions. A letter key follows its question's number, so `Q7A`
  picks choice A of question Q7. A number key follows a hyphen, so
  `Q8-1` picks choice 1 of question Q8.
- Put detail on the reply line, after it, or in a file you link.
  Detail is what each choice leads to, why you recommend one, an
  example, or what stays blocked until the user answers. With several
  questions, give each detail its own paragraph that starts with its
  number, such as "On Q8:".
- Send the questions as plain Markdown, never in a code block, since
  the checker skips code blocks.
- Before you list a question as waiting, check that the user has not
  answered it yet.
- Match each reply to a question by its number, with or without the
  `Q`, so `7A` answers `Q7`. Ask which question a key answers when
  its number names no question that still waits, such as one you
  never asked, or names questions in two messages, which can happen
  after compaction or a handoff. Ask too when a key has no number and
  fits more than one waiting question. A number that names an
  answered question may mean the user changes that answer, so offer
  that as one reading. Ask in this form, with a new number. In a
  reply such as `Q3A 2B`, act on each key that matches one waiting
  question, and act on the others only once the user answers.
- A question that waits on the user is never a side item. Ask it
  again in each update, whole, in this form and with its number,
  until the user answers or the work ends. Never ask it again by its
  name or number alone. When the questions push an update past 20
  lines, keep them whole and move other detail to a file.
- A list where the user picks any number of items, such as a list of
  lessons to save, keeps the form its own skill gives. Give it no
  bold numbered titles, so the checker leaves it alone.

`scripts/check_update.py` checks each question that has a bold,
numbered title: a number without its `Q`, a card key first, fewer
than 2 choices, a choice over 72 characters, a count of `(rec)` other
than one, and no reply line after the last question. Check the rest
yourself, such as at most 3 choices, numbers not used before in the
session, and reply keys that match the questions. The checker cannot
find a question written in another form.

## Approvals, deadlines and reviews

- Before you list an approval or pull request as waiting on the
  user, look up its state now rather than trusting an earlier
  status. Report an item already approved or merged as done.
- An approval that waits on the user is never a side item either.
  Carry it into each update until it is given or the request ends.
- Describe an approval request by its effect, such as which pull
  request it merges in which repository, and tell the user where to
  approve it. Request IDs mean nothing to the user.
- Give each deadline as the time left, worked out from what you know,
  such as "expires in about two hours". Add a clock time only when
  you know the current time.
- When you restate a choice the user made, cite only the user's own
  reasons, and label any reason you add as yours.
- To list pull requests for review, follow the review list in
  [examples.md](examples.md#a-list-of-pull-requests-to-review).
