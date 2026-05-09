# Personal Dev Environment

Shell, editor, and AI tooling configuration. Two independent entry points:

- **`./pde/pde`** -- Install the shell environment (zsh, tmux, neovim, CLI tools).
- **`pde install config`** -- Migrate the shared config set with no sudo; it does not install runtimes, plugins, or profile extras.
- **`pde install ai-tools`** -- Install AI tool configuration and binaries.

## PDE Quick Start

```bash
./pde/pde minimal   # shell, tmux, neovim, CLI tools
./pde/pde full      # everything above + Node, Claude Code, fonts
```

Optional PDE targets live in the Go CLI:

```bash
cd cli && go build -o ~/.local/bin/pde .
pde install config
pde install obsidian
```

See [`pde/README.md`](./pde/README.md) for profiles, bootstrap, and testing.

## AI Tools Quick Start

```bash
cd cli && go build -o ~/.local/bin/pde .
pde install ai-tools
```

Installs planner, Codex, OpenCode, OpenCode inline shim, Pi, Surveil, and Vibe binaries plus repo-managed AI config.

## AI Source Tree

- `ai/AGENTS.md` is the shared workflow default file.
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
| OpenCode | `ai/opencode/` | `~/.config/opencode/{agents,commands}` | OpenCode commands and agents |
| OpenCode Inline Shim | `cli/cmd/opencode-inline-shim/` | `~/.local/bin/opencode-inline-shim` | Local OpenAI-compatible bridge |
| Codex | `ai/codex/skills/` | `~/.codex/skills/` | Prompt-triggered skills |
| Surveil | `surveil/` | `~/.local/bin/surveil` | Task-document gather/research CLI |
| Pi | `ai/pi/agent/` | `~/.local/bin/pi`, `~/.pi/agent/` | Managed CLI plus settings |

The config helper only manages the shared PDE config files under `~/.zshrc`, `~/.zsh_plugins.txt`, `~/.tmux.conf`, `~/.p10k.zsh`, and `~/.config/bottom/bottom.toml`.

The AI installer backs up the managed OpenCode, Codex, and Pi paths before replacement and leaves unrelated root config in place.

## Using OpenCode Commands

In OpenCode, type `/command_name` to invoke. These are the same commands installed from `ai/opencode/commands/`.

| Command | Purpose |
|---------|---------|
| `/design_doc` | Create a technical design document for a feature or system |
| `/create_plan` | Produce a research-backed implementation plan |
| `/review_plan` | Validate a plan for architecture, bugs, and completeness |
| `/implement_plan` | Execute plan phases with verification |
| `/cleanup_plan` | Tear down completed plan worktrees and finish cleanup |
| `/research_codebase` | Document how the codebase works (read-only) |
| `/document_codebase` | Diagnose documentation gaps and fix them at the right level |

## Using Codex Skills

Codex skills are prompt-triggered, not slash commands. Use them by asking naturally or naming the skill explicitly.

| Skill | What it does | Example prompt |
|-------|-------------|----------------|
| `create-plan` | Create a research-backed implementation issue | "Use create-plan to plan the auth refactor" |
| `design-doc` | Draft a technical design document | "Use design-doc to design the new caching layer" |
| `document-codebase` | Audit and improve project documentation | "Use document-codebase to review docs under pde/" |
| `implement-plan` | Execute an approved plan with verification | "Use implement-plan on docs/PDEV-006.md" |
| `cleanup-plan` | Clean up a completed plan worktree safely | "Use cleanup-plan for the finished auth refactor worktree" |
| `research-codebase` | Explain how existing code works | "Use research-codebase to explain how pde install ai-tools works" |
| `review-plan` | Review a plan for architecture, bugs, completeness | "Use review-plan on docs/design-auth.md with focus on security" |

Skills are installed to `~/.codex/skills/`, and the installer copies the shared `AGENTS.md` into `~/.codex/` so the workflow defaults stay aligned with the rest of the tree.

## Requirements

- Linux system with `sudo` access for PDE installs.
- The AI installer expects `go`, `git`, `curl`, `cargo`, and `npm`/`nvm` on Linux.
- `vibe run` additionally expects Docker plus provider auth via env vars or `~/.pi/agent/auth.json`.
