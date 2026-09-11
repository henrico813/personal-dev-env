# Personal Dev Environment

Shell, editor, and AI tooling configuration with two Go applications:

- **`pde-installer`** installs machine, config, Obsidian, and AI tooling.
- **`pde`** manages vault configuration and lookup only.

## PDE Quick Start

Build the installer from an existing repository checkout, then reconcile the
pinned environment:

```bash
mkdir -p ~/.local/bin
go build -C pde-installer -o ~/.local/bin/pde-installer .
export PATH="$HOME/.local/bin:$PATH"
pde-installer install --profile terminal
```

This terminal profile is the small terminal-focused entry point. On a fresh
HOME, omitting `--profile` selects the full profile. An existing install
reuses its saved profile. Only `install` accepts `--profile full|terminal`; the
selected profile is saved in
`~/.config/pde/config.json`. A terminal installation can later expand to full
with `pde-installer install --profile full`. Existing terminal users must run
`pde-installer install --profile full` to expand. A full installation cannot
change to terminal because installed components are not removed.

The installer does not clone or update the repository. Run it from anywhere
inside the checkout, pass `--repo-root /path/to/personal-dev-env`, or set
`PDE_REPO_ROOT`.

The installer exposes five commands:

```bash
pde-installer install
pde-installer update
pde-installer doctor
pde-installer list
pde-installer config
```

- `install` reconciles the selected profile in dependency order. On a fresh
  `HOME`, omitting `--profile` selects full; an existing install reuses its
  saved profile.
- `update` updates saved-profile tools and home configuration.
- `doctor` validates host prerequisites, pins, and managed paths for the saved
  profile (or full on a fresh `HOME`).
- `list` reports ownership and installed status for the saved profile (or full
  on a fresh `HOME`).
- `config` applies saved-profile home configuration without updating tools.

Run `pde-installer update --help` or `pde-installer config --help` before
maintaining an existing installation. See the
[command reference](pde-installer/docs/reference/commands.md) for details.

Mutating commands reject UID 0. The installer uses `sudo apt-get` for missing
Ubuntu dependencies. Other managed files stay below `HOME`. Use `--dry-run` to
preview ordered work without making changes.

Build the separate vault-only CLI when vault commands are needed:

```bash
go build -C cli -o ~/.local/bin/pde .
export PATH="$HOME/.local/bin:$PATH"
pde vault --help
```

The `config` target migrates known vault values from deprecated `paths.env`
state before removing it. It records the selected checkout in `config.json`
and preserves unrelated fields. Future vault changes use `pde vault`.

See [`pde-installer/README.md`](./pde-installer/README.md) for installer details.

## AI Tools Quick Start

```bash
pde-installer install
```

The default full install includes the AI tooling. It installs planner, Codex,
OpenCode, the OpenCode inline shim, Pi, Surveil, and Vibe binaries plus
repo-managed AI config.

## AI Source Tree

- `ai/AGENTS.md` is the shared workflow default file.
- `ai/skills/` holds shared Agent Skills-format guidance.
- `ai/opencode/` holds OpenCode agents and commands.
- `ai/codex/` holds Codex skills.
- `ai/pi/agent/` holds Pi settings and package resources.
- `surveil/` holds the Surveil task-doc CLI docs.
- `pde/AGENTS.md` holds repo-local PDE notes.

## Installed Layout

| Tool | Config source | Install target | Invocation style |
|------|--------------|----------------|-----------------|
| planner | `planner/` | `~/.local/bin/planner` | Shared plan CLI |
| Vibe | `vibe/` | `~/.local/bin/vibe` | Worktree-backed execution harness |
| Behavior-focused testing | `ai/skills/behavior-focused-testing/` | `~/.agents/skills/behavior-focused-testing/`, `~/.codex/skills/behavior-focused-testing/` | Shared test-writing guidance |
| Go development | `ai/skills/go-development/` | `~/.agents/skills/go-development/`, `~/.codex/skills/go-development/` | Shared Go development guidance |
| Rust development | `ai/skills/rust-development/` | `~/.agents/skills/rust-development/`, `~/.codex/skills/rust-development/` | Shared Rust development guidance |
| Git messages | `ai/skills/git-messages/` | `~/.agents/skills/git-messages/`, `~/.codex/skills/git-messages/` | Shared commit and PR guidance |
| Promote memory | `ai/skills/promote-memory/` | `~/.agents/skills/promote-memory/`, `~/.codex/skills/promote-memory/` | Reviewed memory-to-skill promotion |
| OpenCode | `ai/opencode/`, `chezmoi/` | `~/.config/opencode/{agents,commands}`, `opencode.json` permission merge | OpenCode commands and agents |
| OpenCode memory | `ai/AGENTS.md`, `chezmoi/` | `opencode-mem@2.25.0`, `~/.opencode-mem/` | Explicit correction retention |
| OpenCode Inline Shim | `cli/cmd/opencode-inline-shim/` | `~/.local/bin/opencode-inline-shim` | Local OpenAI-compatible bridge |
| Codex | `ai/codex/skills/` | `~/.codex/skills/` | Prompt-triggered skills |
| Surveil | `surveil/` | `~/.local/bin/surveil` | Task research and evidence merge CLI |
| Pi | `ai/pi/agent/` | `~/.local/bin/pi`, `~/.pi/agent/` | Managed CLI plus settings |

Shared configuration lives in `chezmoi/`, including local-file mappings for the complete `ai/` source tree and checksummed remote externals.

The installer snapshots changed chezmoi targets before apply. A scoped modifier merges an XDG-aware `permission.external_directory` allowance for Surveil state into user-owned `opencode.json`; unrelated settings remain in place and failures roll back the snapshot.

OpenCode memory stores local profile data under `~/.opencode-mem/`. When
corrected, OpenCode saves the durable behavior as an explicit profile
preference without requiring the user to organize memory. Automatic transcript
capture and the plugin web server are disabled for this focused integration.
The installer preserves unrelated strict-JSON memory settings and uses global
`git user.email` as the stable profile identity. Commented JSONC fails safely
instead of being overwritten, and the older `opencode-mem.json` filename must
be migrated first. Changing the global email starts a new profile; the email
is stored in local plugin data. This PoC assumes one correction writer at a
time. Project or environment overrides can replace these global settings and
void the privacy guarantees. This integration is OpenCode-only; Codex and Pi
continue without persistent memory. OpenCode downloads the plugin and local
embedding model on first use, which may require network access.

## Using OpenCode Commands

In OpenCode, type `/command_name` to invoke. These are the same commands installed from `ai/opencode/commands/`.

| Command | Purpose |
|---------|---------|
| `/design_doc` | Create a technical design document for a feature or system |
| `/create_plan` | Produce a surveil-backed implementation plan |
| `/review_plan` | Validate a plan for architecture, bugs, and completeness |
| `/implement_plan` | Execute plan phases with verification |
| `/cleanup_plan` | Clean completed plan, worktree, branch, PR evidence, and main state |
| `/research_codebase` | Document how the codebase works (read-only) |
| `/document_codebase` | Diagnose documentation gaps and fix them at the right level |

## Using Codex Skills

Codex skills are prompt-triggered, not slash commands. Use them by asking naturally or naming the skill explicitly.

| Skill | What it does | Example prompt |
|-------|-------------|----------------|
| `create-plan` | Create a surveil-backed implementation issue | "Use create-plan to plan the auth refactor" |
| `design-doc` | Draft a technical design document | "Use design-doc to design the new caching layer" |
| `document-codebase` | Audit and improve project documentation | "Use document-codebase to review docs under pde/" |
| `implement-plan` | Execute an approved plan with verification | "Use implement-plan on docs/PDEV-006.md" |
| `cleanup-plan` | Clean completed plan, worktree, branch, PR evidence, and main state | "Use cleanup-plan for the merged auth refactor branch" |
| `research-codebase` | Explain how existing code works | "Use research-codebase to explain how pde-installer install works" |
| `review-plan` | Review a plan for architecture, bugs, completeness | "Use review-plan on docs/design-auth.md with focus on security" |

Skills are installed to `~/.codex/skills/`, and the installer copies the shared `AGENTS.md` into `~/.codex/` so the workflow defaults stay aligned with the rest of the tree.

## Requirements

- An existing Git checkout and Go are required to build `pde-installer`.
- Ubuntu 22.04 or newer on Linux amd64 is required.
- Use an unprivileged user with `sudo` access for missing apt packages.
- `vibe run` additionally expects Docker plus provider auth via env vars or `~/.pi/agent/auth.json`.

## Installer Tests

Fast Go tests use temporary homes, local HTTP servers, and small executable
fixtures. They run the production APIs without downloading or compiling full
toolchains:

```bash
go test -C pde-installer ./...
go test -C pde-installer -race ./...
go vet -C pde-installer ./...
```

These tests protect the operations most likely to damage an existing setup:
path containment, package-state decisions, exact version checks, installer
locking, durable recovery, backend activation, and rollback after a later
failure.

CI runs one direct smoke test as an unprivileged user with a temporary home and
a fake ChezMoi binary. It rejects a root configuration call, verifies config
idempotency and rollback, and confirms that dry-run install, update, and config
do not mutate the home. It does not build Docker images or install PDE tools.
The GitHub Actions Go toolchain and module cache can download on a cache miss.
Run `./pde-installer/test/verify-ci-smoke.sh` locally.

The full Go suite and Docker checks are local-only verification:
Run `go test -C pde-installer ./...`, `go test -C pde-installer -race ./...`,
`go vet -C pde-installer ./...`, `./pde-installer/test/run-tests.sh smoke`, or
`./pde-installer/test/run-tests.sh terminal` when deliberately testing installer
behavior. A full-profile installation remains a manual real-world check.
