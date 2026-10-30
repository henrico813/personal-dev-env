# Comments readers reject

Read this when you write or review a comment, docstring or test
explanation. Each pattern below made a reader stop and ask what the
comment meant. Fix the comment, or the code when the comment cannot be
fixed without a long explanation.

Readers will also reject comments that aren't written in
[[plain direct english]].

## Contents

- [A variable name used as a word](#a-variable-name-used-as-a-word)
- [A project word left undefined](#a-project-word-left-undefined)
- [A sample name used as if known](#a-sample-name-used-as-if-known)
- [Jargon where a plain word works](#jargon-where-a-plain-word-works)
- [No reason the code exists](#no-reason-the-code-exists)

## A variable name used as a word

A sentence that leans on a parameter or local name makes the reader
look it up.

- Bad: "Returns the answer the owner gave to e."
- Better: "Returns the owner's decision on one comment, in the form the
  checker reads."

## A project word left undefined

A word with a special meaning in this project reads as ordinary
English to a newcomer, so they take the wrong meaning.

- Bad: "A keep puts back the text the owner saw."
- Better: "When the owner keeps a comment, the checker writes back the
  text the owner was shown."

## A sample name used as if known

Names from a test's sample data mean nothing outside the test setup.

- Bad: "An edit to Total's comment."
- Better: "One comment edit in the sample Go file."

## Jargon where a plain word works

Library and system terms slow a reader when a plain word says the
same thing.

- Bad: "Signal on the final unlink of a renamed source temporary."
- Better: "Send a signal when the checker deletes its leftover copy."

## No reason the code exists

A test or helper comment that only repeats what the code does leaves
the reader asking why it is there.

- Bad: "Deliver a real signal or the requested write error."
- Better: "Fail one file save, so the test can check that every file
  goes back."
