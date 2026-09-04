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
pde-installer install
```

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

- `install` and `update` reconcile pkgsrc, Aqua, direct releases, npm, fonts, local builds, and config in order.
- `doctor` validates host prerequisites, pins, and managed paths.
- `list` reports ownership and installed status.
- `config` applies only the chezmoi source.

Mutating commands reject UID 0. Everything is installed below `HOME`; the
installer never invokes apt or sudo. Use `--dry-run` to print ordered work
without changing `HOME`.

Build the separate vault-only CLI when vault commands are needed:

```bash
go build -C cli -o ~/.local/bin/pde .
pde vault --help
```

The `config` target migrates known vault values from deprecated `paths.env`
state before removing it. It records the selected checkout in `config.json`
and preserves unrelated fields. Future vault changes use `pde vault`.

See [`pde/README.md`](./pde/README.md) for target details and Docker tests.

## AI Tools Quick Start

```bash
pde-installer install
```

Installs planner, Codex, OpenCode, OpenCode inline shim, Pi, Surveil, and Vibe binaries plus repo-managed AI config.

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
| Git messages | `ai/skills/git-messages/` | `~/.agents/skills/git-messages/`, `~/.codex/skills/git-messages/` | Shared commit and PR guidance |
| OpenCode | `ai/opencode/`, `chezmoi/` | `~/.config/opencode/{agents,commands}`, `opencode.json` permission merge | OpenCode commands and agents |
| OpenCode Inline Shim | `cli/cmd/opencode-inline-shim/` | `~/.local/bin/opencode-inline-shim` | Local OpenAI-compatible bridge |
| Codex | `ai/codex/skills/` | `~/.codex/skills/` | Prompt-triggered skills |
| Surveil | `surveil/` | `~/.local/bin/surveil` | Task research and evidence merge CLI |
| Pi | `ai/pi/agent/` | `~/.local/bin/pi`, `~/.pi/agent/` | Managed CLI plus settings |

Shared configuration lives in `chezmoi/`, including local-file mappings for the complete `ai/` source tree and checksummed remote externals.

The installer snapshots changed chezmoi targets before apply. A scoped modifier merges an XDG-aware `permission.external_directory` allowance for Surveil state into user-owned `opencode.json`; unrelated settings remain in place and failures roll back the snapshot.

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
- Install host compilers, development headers, archive tools, Git, and curl or fetch before running the installer.
- Installation is rootless and uses a source-only pkgsrc bootstrap under `~/.local`.
- `vibe run` additionally expects Docker plus provider auth via env vars or `~/.pi/agent/auth.json`.

## Installer Tests

Fast Go tests use temporary homes, local HTTP servers, and small executable
fixtures. They run the production APIs without downloading or compiling full
toolchains:

```bash
go test -C pde-installer ./...
go test -race -C pde-installer ./...
go vet -C pde-installer ./...
```

These tests protect the operations most likely to damage an existing setup:
path containment, package-state decisions, one-shot pkgsrc mutations, exact
version checks, installer locking, durable recovery, backend activation, and
rollback after a later failure.

Docker is still used to confirm that the command behaves as an unprivileged
process on supported Ubuntu releases:

```bash
./pde-installer/test/run-tests.sh smoke
```

The smoke runs all five commands as an unprivileged user on Ubuntu 22.04 and
24.04 and confirms that root execution is rejected. It covers config
transactions, including an induced chezmoi failure and rollback. Install and
update are dry runs. A complete installation remains a manual real-world check
because it is too expensive for hosted CI.
