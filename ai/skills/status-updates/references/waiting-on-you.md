# What waits on the user

Read this before you ask the user a question, in an update or on its
own, ask for an approval, or list pull requests to review.

## Ask in the short form

The user answers on a phone. Every question to the user, from any
agent, including a hub that runs other sessions, takes this form, in
an update or on its own:

```markdown
**1. Lessons rework: how the repeat search survives squash merges**
- A: the merge step copies the fix lines into the squash message **(rec)**
- B: merge fix PRs by rebase
- C: search merged PRs on GitHub

**2. Model routing: do you want a routing table and one start command?**
- 1: yes, redesign around them **(rec)**
- 2: partly, plus something else
- 3: no

Reply like `1A 2-1`.
```

- Give each question a bold, numbered title. The title names the
  work in plain words first, then what the user decides. A card key,
  if any, comes after those words in parentheses, never first.
- Number the questions on from any numbered list of actions above
  them in the same message, so each number names one thing.
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
  questions. A letter key follows its question's number, so `1A`
  picks choice A of question 1. A number key follows a hyphen, so
  `2-1` picks choice 1 of question 2.
- Put detail on the reply line, after it, or in a file you link.
  Detail is what each choice leads to, why you recommend one, an
  example, or what stays blocked until the user answers. With several
  questions, give each detail its own paragraph that starts with its
  number, such as "On 2:".
- Send the questions as plain Markdown, never in a code block, since
  the checker skips code blocks.
- Before you list a question as waiting, check that the user has not
  answered it yet.
- A question that waits on the user is never a side item. Ask it
  again in each update, whole and in this form, until the user
  answers or the work ends. Never ask it again by its name alone.
  When the questions push an update past 20 lines, keep them whole
  and move other detail to a file.
- A list where the user picks any number of items, such as a list of
  lessons to save, keeps the form its own skill gives. Give it no
  bold numbered titles, so the checker leaves it alone.

`scripts/check_update.py` checks each question that has a bold,
numbered title: a card key first, fewer than 2 choices, a choice over
72 characters, a count of `(rec)` other than one, and no reply line
after the last question. Check the rest yourself, such as at most 3
choices, numbers that follow the actions, and reply keys that match
the questions. The checker cannot find a question written in another
form.

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
