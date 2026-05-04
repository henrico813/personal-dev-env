# AI Config Source Tree

This directory is the neutral repo-managed source for PDE AI tooling.

- `AGENTS.md` holds shared workflow defaults.
- `opencode/` holds OpenCode agents and commands.
- `codex/` holds Codex skills.
- `pi/agent/` holds Pi settings and any Pi-specific resources.

`pde install ai-tools` installs the Pi CLI separately, then syncs
`pi/agent/` into `~/.pi/agent/` as the managed config source.
Pi extension packages referenced from `pi/agent/settings.json` remain
unmanaged by `ai-tools` in this issue.
The installer copies the shared `AGENTS.md` into each harness config and backs up only the managed paths it replaces.
