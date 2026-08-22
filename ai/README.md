# AI Config Source Tree

This directory is the neutral repo-managed source for PDE AI tooling.

- `AGENTS.md` holds shared workflow defaults.
- `opencode/` holds OpenCode agents and commands.
- `codex/` holds Codex skills.
- `pi/agent/` holds Pi settings and any Pi-specific resources.

`pde install ai-tools` installs planner, `codex`, `opencode`,
`opencode-inline-shim`, `pi`, `surveil`, and `vibe`, then syncs `opencode/`,
`codex/`, and `pi/agent/` into their managed config homes. Pi
extension packages referenced from `pi/agent/settings.json` remain
unmanaged by `ai-tools`. Surveil is installed as a standalone binary;
the installer only adds its XDG-aware state directory to OpenCode's
`permission.external_directory` rules. Vibe relies on provider env vars
or `~/.pi/agent/auth.json` rather than managed config under `ai/`. The
installer copies the shared `AGENTS.md` into each harness config, backs
up managed paths it replaces, and backs up `opencode.json` only when the
permission merge changes it.
