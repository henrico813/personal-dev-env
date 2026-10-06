---
name: git-messages
description: Writes and checks commit messages and pull request titles and descriptions. Use before git commit and before writing or editing pull request text.
---

# Git Messages

Follow the repository's documented style first. Otherwise use these defaults.

1. Use `<type>[(scope)][!]: <summary>` with `feat`, `fix`, `docs`, `test`,
   `refactor`, `perf`, `build`, `ci`, `chore`, `style`, or `revert`.
2. Keep titles to 50 characters, excluding a trailing ` (#N)`, and omit a
   final period. Put a blank line after the title.
3. For a commit body, explain why the change matters, what changed, and any
   limit or risk. Skip the body only when the title says everything.
4. Wrap body lines to 72 characters. For PRs, use `## Overview`, `## Changes`,
   and `## Testing`, in that order. The Overview becomes the squash commit
   body, so it must stand alone (why and what) and pass `msg lint`. Add one
   Changes bullet per change and append a bullet as the PR changes.
5. In Testing, write a short prose block: why the check matters, how to run a
   human-runnable check in a code fence, what to expect, and the real result.
   Report only checks actually run.
6. Review all text for plain direct English and AI slop: would a new reader
   understand every term, does each sentence say something specific, are the
   results real, and can it be shorter?

7. Run `python3 ~/.agents/skills/git-messages/scripts/msg fmt FILE`, then
   `python3 ~/.agents/skills/git-messages/scripts/msg lint [--pr] FILE`.
   `fmt` wraps text and `lint` reports exact counts. Recheck up to three times,
   fixing only reported lines. Repositories may keep their own copy.

Leave out branch-only commit hashes, meaningless test counts, private tracker
references, unexplained project terms, and AI attribution.

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

    This checks that previewing a change remains safe. Run:
    ```bash
    ./tool --dry-run
    ```
    Expect planned file changes and no modified files. Result: I saw the
    preview and the working tree stayed clean.

Bad titles: `fix: update stuff.`; `feat: improve things for everyone`.
