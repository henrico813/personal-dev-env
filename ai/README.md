# AI Config Source Tree

This directory is the neutral repo-managed source for PDE AI tooling.

- `AGENTS.md` holds shared workflow defaults.
- `opencode/` holds OpenCode agents and commands.
- `codex/` holds Codex skills.
- `pi/agent/` holds Pi settings and any Pi-specific resources.

`pde install ai-tools` installs planner, `codex`, `opencode`,
`opencode-inline-shim`, `pi`, and `vibe`, then syncs `opencode/`,
`codex/`, and `pi/agent/` into their managed config homes. Pi
extension packages referenced from `pi/agent/settings.json` remain
unmanaged by `ai-tools`, and `vibe` relies on provider env vars or
`~/.pi/agent/auth.json` rather than managed config under `ai/`. The
installer copies the shared `AGENTS.md` into each harness config and
backs up only the managed paths it replaces.
