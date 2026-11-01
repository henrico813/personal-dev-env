# New-reader checklist

Use this list when a new reader must understand the code, such as
before the owner reads a pull request. A new reader knows the
language but has never seen this repository. Each line names one
check and links the skill section that owns its rule. Read the rule
there; this page does not repeat it.

## Two readers

Give the checks to two fresh reviewers:

- The code-only reader gets only the code and tests. It asks whether
  the code makes sense on its own. `code-documentation` keeps its term
  check code-only, in Editing and review.
- The design-aware reader also gets the PR text, the plan and the
  design's Dictionary section. It asks whether the code matches the
  design.

A delegated reviewer starts no reviewers. It runs every check itself
and reports which code-only checks it could not keep code-only.

## Code-only reader

1. Main flow: the module doc names where to start reading, and those
   functions come first.
   [code-documentation](../../code-documentation/SKILL.md),
   Functions, classes, modules, and comments.
2. Test names: each starts with what is under test and says the
   result a user relies on.
   [behavior-focused-testing](../../behavior-focused-testing/SKILL.md),
   section 3.
3. Test docs: each test says what must stay true and what is lost if
   it breaks, long tests have step comments, and each helper and
   fixture has a one-line doc.
   [code-documentation](../../code-documentation/SKILL.md), Tests.
4. Short comments: within the language's length and colon limits, and
   no sample names.
   [code-documentation](../../code-documentation/SKILL.md), Functions,
   classes, modules, and comments, and Go and Python rules, and
   [comments readers reject](../../code-documentation/references/comments-readers-reject.md).

## Design-aware reader

5. Design words: names, comments and PR text use the Dictionary's
   words with its meanings.
   [code-documentation](../../code-documentation/SKILL.md), Write
   plain, direct English.
6. True claims: every claim in comments and PR text matches the code
   and tests.
   [code-documentation](../../code-documentation/SKILL.md), Keep
   claims true, and [review-plan](../SKILL.md), Step 1.
7. Error codes and type names mean what the design says they mean.
   Same rule as check 5.

## Either reader

8. Tests still catch bugs put back: put back each real bug one at a
   time, in a scratch copy, and check that a test fails.
   [review-plan](../SKILL.md), Step 3.

Report in review-plan's Step 5 format, with `file:line`, the check's
number and the exact fix.
