---
name: code-documentation
description: Use when planning changes to, writing, changing, or reviewing source code or automated tests where comments, docstrings, or inline explanations affect maintainability, onboarding, or correct use.
---

# Code Documentation

Write source documentation that helps a developer unfamiliar with the repository
understand, use, test, and safely change the code.

Assume the reader knows the programming language but does not know this
repository, its design history, project-specific terms, or failure modes. Write
for that reader without labeling them as a beginner or junior in source text.

This skill sets the cross-language baseline. Follow stricter language,
framework, safety, and repository requirements when they apply.

## Decide whether documentation is needed

Do not add documentation by default. Add or update it when it saves meaningful
reader effort, prevents misuse, or is required by the language or repository.

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

## Write plain, direct English

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
5. Preserve good existing text. Do not rewrite documentation only for style,
   including on a repeated pass over the same code.
6. Keep edits within the task's scope.
7. Verify documentation claims against the implementation and tests when
   practical.

For abstract I/O dependencies, document only guarantees established by the
interface and checked behavior. A slice passed to a write method does not prove
that every requested byte was persisted. Do not add parameter, result, or caller
remediation sections unless they answer a supported, useful reader question.

For documentation-only work, do not change behavior, public signatures, test
names, test inputs, or assertions unless the user explicitly requests it.

A valid outcome is no documentation change.

During implementation, make supported in-scope documentation fixes
automatically. During review-only work, report findings without editing unless
the user explicitly asks for fixes.

If a documentation issue reveals unclear requirements or a possible code bug,
report it instead of hiding it with prose.

## Completion check

Before declaring source or test work complete, check:

- changed documentation is still accurate
- important behavior does not require unnecessary tracing
- new prose does not merely repeat the code
- stated reasons are supported
- test explanations match what the assertions prove
- the amount of documentation fits the code's complexity and risk

Fix supported in-scope issues once. Do not start an open-ended prose polishing
loop.
