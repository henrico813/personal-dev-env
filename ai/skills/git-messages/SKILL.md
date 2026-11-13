---
name: git-messages
description: Writes and checks commit messages and pull request titles and descriptions. Use before git commit and before writing or editing pull request text.
---

# Git Messages

Follow the repository's documented style first. Otherwise use these defaults.

1. Use `<type>[(scope)][!]: <summary>` with `feat`, `fix`, `docs`, `test`,
   `refactor`, `perf`, `build`, `ci`, `chore`, `style`, or `revert`.
   Use `docs` only when no behavior changes. Skills, prompts and agent
   instructions control what an agent does, so changing them is `feat`
   or `fix`; `msg lint` refuses `docs` for them.
   Before writing a title, run
   `git config --local --get msg.titlePrefix`. A value is a regex for a
   prefix allowed before the type, which `msg lint` enforces.
2. Keep titles to 50 characters, excluding a trailing ` (#N)`, and omit a
   final period. Put a blank line after the title.
3. For a commit body, explain why the change matters, what changed, and any
   limit or risk. Skip the body only when the title says everything.
4. Wrap body lines to 72 characters. For PRs, use `## Overview`, `## Changes`,
   and `## Testing`, in that order. The Overview becomes the squash commit
   body, so it must stand alone (why and what) and pass `msg lint`.
5. Write PR text for a reader who missed the work, short enough to
   read on a phone. Overview: two sentences on who was affected and
   what changed. Changes: one short line per piece of work, with
   review fixes folded into those lines rather than listed as rounds.
   Testing: one or two sentences of real results, naming the version
   or commit they ran on, with at most one short line of real output,
   such as `20 passed`. For an evaluation, give one concrete example
   with and without the change, and its limits. Add one line on what
   was not done, if anything. End Testing with a checks list in a
   fenced code block. Its first line reads `Checks`. Each line after
   it covers one review, test or try, and starts with `pass`, `fail`
   or `not run`. A try is a run of the changed behavior where it is
   deployed. A `not run` line gives the reason. Mark a check `pass`
   or `fail` only when it ran. Never refer to commits the squash
   removes.
6. Leave out branch-only commit hashes, meaningless test counts, private
   tracker references, unexplained project terms, and AI attribution.
   Never name a private machine, the user's employer or work, private
   vaults, personal information or secrets, here or in a branch name;
   describe the change in general terms.
7. Follow [plain-direct-english](../plain-direct-english/SKILL.md) and
   review all text for AI slop: would a new reader
   understand every term, does each sentence say something specific, are the
   results real, and can it be shorter?

8. Run `python3 ~/.agents/skills/git-messages/scripts/msg fmt FILE`, then
   `python3 ~/.agents/skills/git-messages/scripts/msg lint [--pr] FILE`.
   `fmt` wraps text and `lint` reports exact counts. For a stacked PR,
   add `--base PARENT` to `lint` so it counts only this PR's files.
   Recheck up to three times, fixing only reported lines. Repositories
   may keep their own copy.

Example commit:

    fix: preserve dry-run output

    Dry-run should show planned file changes without writing them.
    Print the same summary users will review before applying it.

Example pull request:

    fix: preserve dry-run output

    ## Overview

    Dry-run now shows planned file changes without writing them.
    Users can review the result before applying it.

    ## Changes

    - Show planned file changes when the dry-run flag is set.
    - Keep dry-run from writing files.

    ## Testing

    On version 2.3.0, `./tool --dry-run` listed the planned file
    changes and the working tree stayed clean.

    ```
    Checks
    pass  tests (20 passed)
    pass  dry-run try on version 2.3.0
    not run  Windows try: no Windows machine
    ```

Bad titles: `fix: update stuff.`; `feat: improve things for everyone`.
