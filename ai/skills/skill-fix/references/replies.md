# Sync replies and held fixes

`skill-fix commit` and `skill-fix sync` report how the push went in
a `sync` field. `skill-fix sync` prints a `repos` list instead, with
one entry per skills repository, each with its own `sync` field.

## Contents

- What each sync state means
- A conflict with another PC's fix
- Held fixes
- The state of each fix pull request

## What each sync state means

| `sync` | What to do |
| --- | --- |
| `pushed` or `pr_open` | Nothing: the fix is on the open fix pull request, number `pr` |
| `pr_waiting` | Nothing: the request to open the fix pull request waits for the user's approval |
| `offline` | Nothing: the fetch failed, and a later sync pushes the fix |
| `scan` | Tell the user that `ci/scan` refused the push, with the file and line it printed; the fixes stay unpushed |
| `conflict` | Follow "A conflict with another PC's fix" below |
| `skipped` | Nothing: this repository has nothing to push, or is not set up on this PC |
| `error` | Tell the user the entry's `detail` once; the fixes stay on their branch for a later sync |

`skill-fix sync` with no options syncs every repository this PC has.
Add `--repo personal` or `--repo extra` to sync one.

## A conflict with another PC's fix

`conflict` means a fix on this PC and a change on `main` edit the
same lines. The fixes are safe on their branch. Run `skill-fix
status`, and find the repository whose `conflict` is true.

When its `pr_state` is `open`, merge `main` into the fix branch in
the fix worktree:

```bash
remote=$(git -C <worktree> config branch.main.remote || echo origin)
git -C <worktree> merge "$remote/main"
```

Resolve each conflict so every fix keeps the meaning of its
`Intent:` line, which `git -C <worktree> log` shows. Show the user
the resolved diff. On the user's yes, run `git -C <worktree> commit
--no-edit`, then `skill-fix sync`.

When `pr_state` is `merged`, tell the user, and leave the fix branch
as it is.

## Held fixes

A held fix stays on a local branch of the personal repository and is
never pushed. Each has a number:

```bash
skill-fix held            # list each held fix's number and skill
skill-fix held show <n>   # print its diff for the user to read
skill-fix held drop <n>   # delete it
skill-fix held retry <n>  # check it again and commit it if it passes
```

Run `show` only when the user asks to see the fix, since its text
may hold an extra term. Never copy its output into a message, a file or
a commit. `retry` needs a fix worktree with no edits; run
`skill-fix start <skill>` first.

## The state of each fix pull request

`skill-fix status` prints, for each repository, the fix branch, the
number of commits not pushed yet, the fix pull request's number and
state, whether it conflicts, and the number of held fixes. It
changes nothing.
