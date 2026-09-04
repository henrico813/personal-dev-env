# AI Config Source Tree

This directory is the neutral repo-managed source for PDE AI tooling.

- `AGENTS.md` holds shared workflow defaults.
- `skills/` holds shared Agent Skills-format guidance.
- `opencode/` holds OpenCode agents and commands.
- `codex/` holds Codex skills.
- `pi/agent/` holds Pi settings and any Pi-specific resources.

`pde-installer install` installs planner, `codex`, `opencode`,
`opencode-inline-shim`, `pi`, `surveil`, and `vibe`, then installs each
package under `skills/` to `~/.agents/skills/<name>/` and
`~/.codex/skills/<name>/`. It syncs `opencode/`, `codex/`, and
`pi/agent/` into their managed config homes. Pi
extension packages referenced from `pi/agent/settings.json` remain
unmanaged by the installer. Surveil is installed as a standalone binary;
the installer uses the repository's scoped chezmoi modifier to add its XDG-aware state directory to OpenCode's
`permission.external_directory` rules. Vibe relies on provider env vars
or `~/.pi/agent/auth.json` rather than managed config under `ai/`. The
installer copies the shared `AGENTS.md` into each harness config, backs
up managed paths it replaces, and backs up `opencode.json` only when the
permission merge changes it.

OpenCode loads the pinned `opencode-mem@2.25.0` plugin. A chezmoi modifier
preserves unrelated strict-JSON settings in `opencode-mem.jsonc`, requires
global `git user.email` for stable profile identity, and disables prompt
storage, automatic capture, and the local web server. Commented JSONC is
rejected without changing the target, and an existing `opencode-mem.json` must
be migrated before installation. The plugin keeps memory under
`~/.opencode-mem/`; PDE does not edit or organize its runtime data. The email is
stored in that local data, changing it starts a new profile, and this PoC
supports one correction writer at a time.

Shared workflow instructions tell OpenCode to read the profile before its first
substantive response and save explicit behavior corrections immediately.
OpenCode downloads the plugin and local embedding model on first use. The inherited
`memory` tool also exposes project-memory operations outside this focused flow.
Project `.opencode/opencode-mem.*` files and OpenCode environment configuration
have higher precedence; using them voids PDE's pin and privacy guarantees.

This first integration guarantees no Codex or Pi memory parity. Their shared
instructions apply the memory workflow only when a `memory` tool is available.

The `promote-memory` skill turns an explicitly requested learning into a
reviewed source change. It targets this repository by default from any
checkout, installs skills through checksummed chezmoi externals, and opens a
pull request with Git author email `henryco4388@gmail.com`.
