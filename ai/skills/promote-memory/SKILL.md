---
name: promote-memory
description: Use when the user asks to promote a saved behavioral learning or memory preference into source-controlled guidance.
---

# Promote Memory

Turn one explicitly requested learning into the smallest reviewed source
change. A memory write never authorizes this work on its own.

## Scope

- Default scope is global. Work in a `personal-dev-env` checkout and open a
  pull request against `henrico813/personal-dev-env`, regardless of the
  current repository.
- Edit the current repository only when the user explicitly asks for
  repository-local guidance.
- Create a branch and worktree from `main`; never commit directly to `main`.

## Destinations

Choose the narrowest one:

- Update an existing `ai/skills/<name>/SKILL.md` when the learning overlaps an
  existing skill.
- Create a new `ai/skills/<name>/SKILL.md` for a reusable conditional
  behavior. Use lowercase kebab-case directories and matching frontmatter
  names.
- Add one short routing pointer in `ai/AGENTS.md`. Keep detailed procedures in
  the skill; `ai/AGENTS.md` is loaded every session and must stay small.

## Workflow

1. Restate the learning as one concise, future-facing rule.
2. Search `ai/skills/` for overlap before creating a package.
3. Write the skill and install it through
   `chezmoi/.chezmoiexternal.toml` entries for
   `.agents/skills/<name>/SKILL.md` and `.codex/skills/<name>/SKILL.md`,
   each with a SHA256 checksum.
4. When `ai/AGENTS.md` changes, recompute and update every `AGENTS.md`
   checksum in the external file.
5. Document the skill in the `README.md` installed-layout table and
   `ai/README.md`.
6. Run `(cd pde-installer && go test ./... && go vet ./...)` and
   `git diff --check`.
7. Before committing, verify the Git author email is exactly
   `henryco4388@gmail.com`. Stop and report a mismatch.
8. Load and follow `git-messages`, commit, push to `github`, and open a PR.

## Example

Learning: "Do not use nested bullets in responses."

`ai/AGENTS.md`:

    - Follow the `response-formatting` skill for list formatting.

`ai/skills/response-formatting/SKILL.md`:

    # Response Formatting

    Use flat lists. Do not nest bullets.
