# AI Config Source Tree

This directory is the neutral repo-managed source for PDE AI tooling.

- `AGENTS.md` holds shared workflow defaults.
- `opencode/` holds OpenCode agents and commands.
- `codex/` holds Codex skills.
- `pi/agent/` holds Pi settings and any Pi-specific resources.

The installer copies the shared `AGENTS.md` into each harness config root and backs up any existing managed config before replacement.
