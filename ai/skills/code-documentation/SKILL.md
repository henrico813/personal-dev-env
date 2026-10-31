---
name: code-documentation
description: Use when planning changes to, writing, changing, or reviewing source code or automated tests where comments, docstrings, inline explanations, or function names affect maintainability, onboarding, or correct use.
---

# Code Documentation

Write source documentation that helps a developer unfamiliar with the repository
understand, use, test, and safely change the code.

Assume the reader knows the programming language but does not know this
repository, its design history, project-specific terms, or failure modes. Write
for that reader without labeling them as a beginner or junior in source text.

This skill sets the cross-language baseline. Follow stricter language,
framework, safety, and repository requirements when they apply.

## Edit gate

Unless documentation is required by an applicable language or repository rule,
edit only when a specific, supported fact needed for safe use or maintenance is
missing, false, or unreasonably hard to discover. Otherwise make no source
change, even when asked to "improve" documentation. Prose derived directly from
a name, signature, single statement, or a test's name, inputs, and assertions is
a restatement and must not be added. Accurate existing documentation need not be
exhaustive: do not revise it merely to add another true detail, section, or
phrasing. Once accurate documentation covers the needed fact, repeating the
same request against unchanged code must produce no further edit.

Look more closely when code has:

- public behavior that is not clear from the signature
- important side effects, errors, retries, rollback, or partial failure
- state transitions, ownership, lifetime, concurrency, or ordering rules
- surprising branches, defensive checks, compatibility behavior, or workarounds
- non-obvious units, limits, defaults, or values whose origin matters
- behavior that requires following several callers or helpers to understand
- tests whose importance is not clear from the name, inputs, and assertions

Function length is only a signal to inspect more carefully. Do not use line
thresholds, comment quotas, or mandatory docstrings for private helpers.

A short function with dangerous side effects may need more explanation than a
long but straightforward function.

## Scale to complexity

Use the smallest explanation that lets the reader work safely.

- **Simple and obvious:** usually add nothing.
- **Moderate:** use a short docstring or comment for behavior, side effects,
  failures, or assumptions that are not obvious.
- **Complex or high-risk:** use a useful function, class, or module docstring
  plus targeted comments at the branches, phases, or transitions that need
  local context.

Do not turn a large function into a wall of prose.

When a comment spends most of its words on how the code works, rather
than why it exists or what can go wrong, simplify the code instead.
Split it, name its steps, or move setup into named helpers. Tests are
the usual case. In review-only work, report it.

## Write plain, direct English

Load and follow the `plain-direct-english` skill whenever you write or
review a comment, docstring or doc.

Prefer short sentences and concrete words. Write for comprehension, not for
sounding formal.

Avoid vague filler such as:

- "This function is responsible for..."
- "This method facilitates..."
- "This provides a robust mechanism for..."
- "This seamlessly handles..."

Describe the actual behavior instead.

Bad:

```python
def save_config(path, data):
    """Provides a robust mechanism for persisting configuration data."""
```

Better when the behavior matters:

```python
def save_config(path, data):
    """Replace the config without leaving a partial file after a failed write."""
```

Technical terms are fine when they are the clearest words. Explain
project-specific terms when a new contributor would not know them. Do not repeat
type information that is already obvious from the signature.

When writing or reviewing comments, check each one against
[references/comments-readers-reject.md](
references/comments-readers-reject.md),
which lists what readers reject, each with a bad and a better example.

## Keep claims true

Support documentation with the code, tests, requirements, existing docs, or
relevant history.

Do not invent a reason because one sounds plausible. If the reason for a strange
value or design choice is unknown, describe useful observable behavior or report
the uncertainty.

Do not turn a suspected bug into documented intended behavior. If documentation
disagrees with the implementation or tests, surface the disagreement.

## Functions, classes, modules, and comments

For public or exported APIs, follow language and repository conventions. Explain
what a caller needs to use the API correctly: important behavior, parameter
meaning, results, side effects, failures, ordering, ownership, or state rules.

Do not start a doc comment with the name it documents, such as
`// Total returns ...`, in any language; the one exception is a Go
package comment, which starts with `Package <name>`.

Private functions do not need docstrings merely because they exist.

Use inline comments for local context such as:

- ordering that must not change
- a branch that protects an important edge case
- a workaround for an external limitation
- a state transition that is easy to misuse
- a value whose unit or source is not obvious

Do not narrate statements.

Bad:

```python
# Increment the retry count.
attempts += 1
```

Useful:

```python
# Do not retry after a partial write; the device may already contain new data.
if wrote_any_bytes:
    return PARTIAL_WRITE
```

## Function names

These rules cover functions and methods that the change adds or
renames, and their bodies. They do not cover test functions, which
follow `behavior-focused-testing`, or names required by the language,
a framework, or an implemented interface, such as `main`, `String()`,
`__repr__` or `Drop::drop`.

Apply DAMP: Descriptive And Meaningful Phrases to names and function
bodies.

Every function name must contain at most seven words. Count snake_case
and camelCase/PascalCase word boundaries; an acronym counts as one
word. Do not evade the limit through obscure abbreviations or
concatenation.

Name actions with verbs first, such as `parse_config` and
`write_report`. Name boolean queries clearly, such as `is_empty`.
Ordinary getters omit `get_`: `report.title()`. Use `as_` for cheap
views, `to_` for conversions that do work, and `into_` for conversions
that consume the value. Use the language's casing: `ParseConfig` in
Go.

```text
parse_config
write_report
is_empty
to_json
ParseConfig
```

Name what the function does or returns, not merely its input or type.
Avoid vague names such as `check` and `save`. Use module or class
context to avoid repetition while keeping call sites understandable.
Put additional detail in the body.

## Tests

Make a test understandable from its name, visible inputs, and assertions first.

Add a test docstring or comment only when a developer still would not know what
behavior the test protects or why breaking it matters.

Do not describe obvious Arrange, Act, or Assert steps. Do not claim more
protection than the assertions establish. Explain shared suite context once
instead of repeating it above every test.

If the test is already clear without prose, no docstring is required.

## Editing and review

When source code or tests change:

1. Identify the concrete reader question that the code, types, names, and tests
   do not already answer. If there is none, do not add or rewrite prose.
2. Read each changed function, class, module, or test as a complete unit,
   including existing comments and docstrings that were not edited.
3. Check whether existing documentation became false or incomplete.
4. Add only useful missing context and remove new filler or narration.
5. Treat documentation already present in the working tree, including
   uncommitted text, as existing documentation. Preserve it when it is accurate
   and already resolves the identified reader question. A repeated generic
   request is not new evidence that more detail is needed and must not by itself
   cause another edit. On a later pass over unchanged behavior, edit existing
   prose only to fix a specific false or unsupported claim, an omission that
   makes it misleading for safe use, or an applicable convention violation;
   otherwise leave it byte-for-byte unchanged. Do not broaden, reformat, or
   polish it merely because more implementation details could be stated.
   The owner is the person who approved text in `.code-comments.json`
   at the repository root, or at the main checkout's root when you
   work in a worktree. Never change that text or those files, even
   when the text seems false; report it instead. The exceptions: run
   the checker's `--approve` when the owner asks you to, and its
   `--merge-approvals` as the `cleanup-plan` skill says.
6. Keep edits within the task's scope.
7. Verify documentation claims against the implementation and tests when
   practical.

Write and review comments after behavior, tests, and names have
settled, because a later rename or behavior change makes reviewed
comments false. When one task changes all of these, do them in this
order: behavior and tests, then names, then comments, then the owner's
review of the comments, last.

For abstract or injected I/O dependencies, distinguish method calls from their
effects. Do not infer storage, mapping, durability, verification, or rollback
guarantees from method names. A slice bound does not prove the slice contains
that many items, and passing data to a write-like method does not prove how much
was accepted, persisted, left behind, or later restored. State stronger effects
only when the interface and checked behavior establish them; otherwise describe
only the call order or attempted operation. Do not add parameter, result, or
caller remediation sections unless they answer a supported, useful reader
question.

When the useful fact is failure ordering, document that fact without implying a
completed write or prescribing unsupported caller action.

Bad:

```python
"""Writes the first chunk before validation. Callers must repair partial output."""
```

Better:

```python
"""Validation follows the call to ``writer.write``, so failure does not mean no write was attempted."""
```

For documentation-only work, do not change behavior, public signatures, test
names, test inputs, or assertions unless the user explicitly requests it.

A valid outcome is no documentation change.

During implementation, make supported in-scope documentation fixes
automatically. During review-only work, report findings without editing unless
the user explicitly asks for fixes.

When a subagent checks whether terms make sense to a new contributor,
give it only the code and tests, not the design doc or plan. Check
claims against requirements yourself.

If a documentation issue reveals unclear requirements or a possible code bug,
report it instead of hiding it with prose.

## The owner's comment review

To put code comments in front of the owner for review, load and
follow the `comment-review` skill. Use it also to change a comment
the owner approved, even when a plan says to; never edit that
comment in the code.

## Comment checker

For Go or Python files whose comments you write or review, run the
checker in this skill's `scripts/` folder, with Python 3.10 or later:

```bash
python3 <this skill's folder>/scripts/check_comments.py \
  --repo <repository root> --base <commit your task started from> \
  <Go or Python files>
```

Each finding names the rule it checks, and the output ends by quoting
that rule's sentence from this skill. Act on each finding:

- `approved`: you changed, removed, or put back a comment the owner
  approved or cut. Restore the owner's text. This fails the run.
  If your task renamed the code, keep the owner's text on the new
  name, changing only the renamed identifiers, and report the finding.
  In your final message, list each rename as an exact
  `--rename OLD=NEW` pair for the owner's `--approve`, quoting names
  with spaces, such as `--rename 'const Red=const Crimson'`. A gone
  comment's finding may add `possibly now <name> at <line>` with the
  pair to record. List a hinted pair only if your task made that
  rename.
- `filler` or `avoided-word` in a sentence your task added or changed:
  fix the sentence. This fails the run. `avoid` in
  `.code-comments.json` gives the word to use instead.
- `restated-name`: reported, never failed.
- `seven-words`, `starts-with-name`, `comment-length` or
  `comment-colon` on a Go name or comment your task added or changed:
  rename the function or fix the comment. This fails the run.
- `verb-first`: reported, never failed. Rename the function when the
  word is not a verb, and when it is, keep the name and list the
  finding in your final message.
- `filler`, `avoided-word` or a Go comment rule marked `report only`:
  the sentence is approved text, a license header, or was there
  before your task.
  List it in your final message, and edit it only as step 5 of Editing
  and review allows.

Run the checker again until it exits 0. If it still fails only with
`approved comment is gone` or `approved file is gone` because your
task renamed, moved or deleted that code, keep the owner's text and
list those findings in your final message for the owner.

If Python cannot read a file, fix its syntax, or report it when the
file needs a newer Python than the checker's `python3`. Never edit
approved text to get past it.

Only the owner runs the checker with `--approve`, or an agent the
owner asked to. It rewrites `.code-comments.json` itself, so no one
types entries by hand. Pass `--rename OLD=NEW` once for each renamed
comment, so its approved or cut entry moves to the new name, and a
pair of paths, relative to `--repo`, for a moved file. For many
renames, put one `--rename=OLD=NEW` per line in a file and pass
`@<file>`.

## Go and Python rules

In Go and Python, these rules apply on top of the ones above:

- Start every function and method name with a verb, except a test, a
  fixture or a property.
- Keep each `//` or `#` comment to at most two sentences.
- Use no colon in a `//` or `#` comment, except inside code or a URL.

In Go, `main`, `init`, getters with no arguments and names that
start with `New` need no verb, and a `Deprecated:` or `BUG(who):`
note may keep its colon and length. The comment checker checks Go
files for these rules, the seven-word limit and doc comments that
start with their name, and with `--base` only new names and comments.

## Python rules

In Python, these rules apply on top of the ones above. Where they
differ, as for docstrings on private helpers and clear tests, these
win:

- Give every function, method and class a docstring, tests and
  fixtures included.
- Start each docstring with a one-line summary that fits on its first
  line.
- Follow a test's summary with a paragraph on why the test matters.
- Never invent a docstring section, such as `Why it matters:`; write a
  plain paragraph instead.

For Python files you add or change, run the rule checker in this
skill's `scripts/` folder:

```bash
python3 <this skill's folder>/scripts/check_python_rules.py \
  --repo <repository root> --base <commit your task started from> \
  <Python files>
```

It needs Python 3.10 or later. It checks only code your task added
or changed. Changing any line of an old function makes the checker
ask for its docstring, so add one. Leave docstrings on old functions
you did not touch for a separate change. Move files with `git mv`, so
the checker compares a moved file with its old path. Each
finding names the rule it checks, and the output ends by quoting that
rule's sentence. Fix each finding and run the checker again until it
exits 0. `verb-first` is reported, never failed: rename the function
when the word is not a verb, and when it is, keep the name and list
the finding in your final message.

## Completion check

Before declaring source or test work complete, check:

- changed documentation is still accurate
- important behavior does not require unnecessary tracing
- new prose does not merely repeat the code
- stated reasons are supported
- test explanations match what the assertions prove
- the amount of documentation fits the code's complexity and risk
- for Go or Python files, the comment checker exits 0, or fails only
  on renamed, moved or deleted approved comments that you reported
- for Python files, the rule checker exits 0, and each `verb-first`
  finding is renamed or, for a real verb, listed in your final message

Fix supported in-scope issues once. Do not start an open-ended prose polishing
loop.
