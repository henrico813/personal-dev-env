# AI Config Source Tree

This directory is the neutral repo-managed source for PDE AI tooling.

- `AGENTS.md` holds shared workflow defaults.
- `skills/` holds shared Agent Skills-format guidance.
- `opencode/` holds OpenCode agents and commands.
- `codex/` holds Codex skills.
- `pi/agent/` holds Pi settings and any Pi-specific resources.

`pde install ai-tools` installs planner, `codex`, `opencode`,
`opencode-inline-shim`, `pi`, `surveil`, and `vibe`, then runs the same
configuration transaction as `pde install ai-config`. The configuration-only
target does not rebuild or install runtimes.

Both targets install every valid package under `skills/` to
`~/.agents/skills/` and `~/.codex/skills/`, and install each package under
`codex/skills/` to `~/.codex/skills/`. Package directories and frontmatter
names must use matching lowercase kebab-case names. PDE tracks owned package
names in `~/.local/share/pde/ai/skill-ownership.json`. On the first managed
run, an existing same-name package is adopted only when its repository-managed
content matches the source; a differing package stops installation without
changing active configuration.

Configuration is preflighted and staged before activation. Existing managed
paths and stale owned packages move to timestamped recovery directories under
`~/.local/share/pde/ai/ai-config-backups/`, outside harness discovery roots.
Unrelated packages in both skill roots remain active. The generated
`create-plan/bin/planner` link is restored from the installed Planner runtime.
Pi
extension packages referenced from `pi/agent/settings.json` remain
unmanaged by `ai-tools`. Surveil is installed as a standalone binary;
the installer uses the repository's scoped chezmoi modifier to add its XDG-aware state directory to OpenCode's
`permission.external_directory` rules. Vibe relies on provider env vars
or `~/.pi/agent/auth.json` rather than managed config under `ai/`. The
installer copies the shared `AGENTS.md` into each harness config, backs
up managed paths it replaces, and backs up `opencode.json` only when the
permission merge changes it. `self-improvement` guides reviewed changes in
the resolved personal-dev-env checkout; it adds no custom command and does
not let memory edit active behavior.

### Optional Hindsight Memory

Hindsight remains separately managed. Install Node 22 or newer, `npx`, `uvx`,
the OpenCode and Codex CLIs, and configure an extraction provider or supported
provider credential. Then install the pinned integration and local daemon:

```bash
npx @vectorize-io/hindsight-coding-agents@0.5.1 \
  install opencode codex --server daemon
```

Set the approved checkout and daemon version in
`~/.hindsight/coding-agent.json`:

```json
{
  "serverMode": "daemon",
  "autoUpdate": false,
  "optInOnly": true,
  "optInPaths": ["/absolute/path/to/personal-dev-env"],
  "codebaseSurvey": false,
  "pageTriggerType": "manual",
  "embedVersion": "0.9.2"
}
```

This pins the 0.5.1 integration and 0.9.2 daemon, but npm and Python transitive
dependencies, harnesses, models, and providers remain externally resolved.
Start a new harness session after installation or configuration changes.
Verify the daemon and repository synchronization:

```bash
curl -fsS http://127.0.0.1:9077/health
uvx hindsight-embed@0.9.2 daemon --profile coding-agent status
node ~/.hindsight/coding-agents/dist/status.js --repo "$PWD" --harness opencode
node ~/.hindsight/coding-agents/dist/status.js --repo "$PWD" --harness codex
```

Use `hindsight_diagnose` and `hindsight_sync_status` inside a harness. If the
daemon is unavailable, agent work continues without memory. Reverse the setup
without deleting retained data:

```bash
npx @vectorize-io/hindsight-coding-agents@0.5.1 uninstall opencode codex
uvx hindsight-embed@0.9.2 daemon --profile coding-agent stop
```
