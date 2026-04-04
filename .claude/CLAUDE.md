## Core Principles

- Never use emojis.

## Issues, Plans, Design Docs, and Research Docs

All plans, issues, design docs, and research docs created by Claude Code for this project should go to the notes vault, $VAULT.

Before writing any file to the vault, read `Projects/AGENTS.md` for filename prefixes, frontmatter, and template requirements.

**Design docs** describe what to build and why. **Implementation plans** (created by `/create_plan`) describe how to execute the work: ordered phases, steps, Definition of Done, and verification. A design doc is input to `/create_plan`, not a substitute for it. Never use `/implement_plan` without a dedicated implementation plan.

**Implementation plans are issues, not design docs.** File them with the `XXXX-###` issue naming convention, issue frontmatter (`type: issue`, `status: open`), and the project's service code (e.g. `ABCD-002 Rename database.py to events.py.md`). Never use `DESIGN-` or `PLAN-` prefixes for implementation plans.

Never commit these files to a PR branch. Issue docs, notes, and tracking files belong in the vault, not the repo.

## Git Workflow

- **Before merging any PR, verify the exact files in the diff** using `gh api repos/.../pulls/N/files`. Never merge if unexpected files are present.
- **Never use `gh pr merge` without `--squash`.** Always squash merge to keep history clean.

## Commit Authorship

- Never add Claude as a commit author.
- Never add Co-Authored-By lines mentioning Claude or Anthropic.
- Always commit using default git settings.

## Code Comments and Docstrings

- Be extremely concise by default, unless told otherwise.
- Prefer examples over prose.
- Assume technical competence.
- Default to 1-2 sentence explanations.
