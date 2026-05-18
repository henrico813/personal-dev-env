# AI Workflow Defaults

- Never use emojis.
- Keep code comments and docstrings concise.
- Prefer examples over prose.
- Use conventional commits.
- Do not add AI attribution to commits.

## Planning Docs

- Plans, issues, design docs, and research docs belong in the default PDE vault resolved through `pde vault path default` or `pde vault locate`.
- Read `Projects/AGENTS.md` before writing to the vault.
- Implementation plans are issues, not design docs.
- Back up managed config before replacing it.
- Resolve plan and vault references with this contract:
- 1. If the user-provided reference is an existing filesystem path, use it directly.
- 2. Track any explicit vault selector from the request: `default`, `main`, or `work`.
- 3. For existing plans, docs, and notes, resolve with `pde vault locate --json --vault <selector> "<reference>"`.
- 4. Use `pde vault path <selector>` only when determining a destination root for a new document or when the user explicitly asks for a vault root.
- 5. Ask only on `ambiguous`, `not_found`, or a real setup `error`.
- Do not use `pde vault default get` as a path lookup step. It returns the selector, not the resolved filesystem path.

## Git Workflow

- Verify the exact files in a PR diff before merging.
- Use squash merges only.

## Code Comments

- Be concise.
- Prefer code over prose.
