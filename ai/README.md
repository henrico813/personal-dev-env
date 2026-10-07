# AI Config Source Tree

This directory is the neutral repo-managed source for PDE AI tooling.

- `AGENTS.md` holds shared workflow defaults.
- `evals/` holds manual AI workflow checks.
- `skills/` holds shared Agent Skills-format guidance, including the shared
  workflow skills.
- `opencode/` holds OpenCode agents and commands.
- `pi/agent/` holds Pi settings and any Pi-specific resources.

Shared skills include `git-messages` for commit and PR text, `pull-request` for PR procedure, `behavior-focused-testing` for automated tests,
`code-documentation` for proportional source explanations, `go-development`
for Go code, `obsidian-zettel` for template-aligned vault notes,
`python-development` for Python code, and `rust-development` for Rust code.
Shared workflow skills include `create-plan` for implementation proposals,
`review-plan` for plan review, `implement-plan` for plan execution,
`cleanup-plan` for plan teardown, `design-doc` for design documents, and
`research-codebase` for as-is codebase research; they install only to
`~/.agents/skills/`, and the OpenCode commands load them. Shared instructions
require skill selection after reading supplied context and before domain work.
Run `evals/skill-routing.md` after changing routing
instructions, skill descriptions, or planning workflows. Run
`evals/plan-workflows.md` after changing planning skills or commands. CI runs
`tests/test_prompt_commands.py` to check the planner command text in the
shared workflow skills. Run it locally with `cd ai && uv run pytest`.

`pde-installer install full` installs planner, `codex`, `opencode`,
`opencode-inline-shim`, `pi`, and `vibe`, then installs each
mapped package under `skills/` to `~/.agents/skills/<name>/`, including the
shared workflow skills. It syncs `opencode/` and
`pi/agent/` into their managed config homes. Pi
extension packages referenced from `pi/agent/settings.json` remain
unmanaged by the installer. Vibe relies on provider env vars
or `~/.pi/agent/auth.json` rather than managed config under `ai/`. The
installer copies the shared `AGENTS.md` into each harness config, backs
up managed paths it replaces, and backs up `opencode.json` only when the
permission merge changes it.

OpenCode memory is intentionally unsupported with the managed Claude adapter.
The chezmoi modifier removes `opencode-mem` from the OpenCode plugin list;
existing `~/.opencode-mem/` data remains on disk but is not loaded. Shared
memory instructions apply only when a memory tool is available; this
installation does not configure one for OpenCode.

The `promote-memory` skill turns an explicitly requested learning into a
reviewed source change. It targets this repository by default from any
checkout, installs skills through checksummed chezmoi externals, and opens a
pull request with Git author email `henryco4388@gmail.com`.
