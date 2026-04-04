# Personal Dev Environment

Shell, editor, and AI tooling configuration. Two independent entry points:

- **`./pde/pde`** -- Install the shell environment (zsh, tmux, neovim, CLI tools).
- **`./install.sh`** -- Install AI tool configuration (Claude Code, OpenCode, Codex).

## PDE Quick Start

```bash
./pde/pde minimal   # shell, tmux, neovim, CLI tools
./pde/pde full      # everything above + Node, Claude Code, fonts
```

See [`pde/README.md`](./pde/README.md) for profiles, bootstrap, and testing.

## AI Config Quick Start

```bash
./install.sh
```

Installs configuration for three AI coding tools from this repo.

## AI Scope Model

This repo is installer-first for AI tooling:

- Root [`.claude/`](/home/justin/Projects/personal-dev-env/.claude) is the repo-managed source for Claude Code global instructions, hooks, scripts, and slash commands
- Root [`.opencode/`](/home/justin/Projects/personal-dev-env/.opencode) is the repo-managed source for OpenCode-compatible commands and agents
- Root [`.codex/`](/home/justin/Projects/personal-dev-env/.codex) is the repo-managed source for Codex skills
- Project-local instruction files such as [`pde/CLAUDE.md`](/home/justin/Projects/personal-dev-env/pde/CLAUDE.md) describe facts and constraints specific to one repo

Use the root AI config trees for workflow defaults that should follow you across repos. Use project-local instruction files for repo-specific implementation facts, commands, and exceptions.

### Installed Layout

| Tool | Config source | Install target | Invocation style |
|------|--------------|----------------|-----------------|
| Claude Code | `.claude/` | `~/.claude/` | Slash commands: `/command_name` |
| OpenCode | `.opencode/` | `~/.config/opencode/` | Same commands, OpenCode-compatible |
| Codex | `.codex/skills/` | `~/.codex/skills/` | Prompt-triggered skills (see below) |

The install script preserves existing Claude user data (`projects/`, `.credentials.json`, `history.jsonl`).

The install behavior is intentionally asymmetric:

- Claude receives the full repo-managed config tree under `~/.claude/`
- OpenCode receives its repo-managed commands and agents under `~/.config/opencode/`
- Codex receives repo-managed skills under `~/.codex/skills/`
- Codex also links `~/.codex/AGENTS.md` to `~/.claude/CLAUDE.md` so Codex and OpenCode both consume the same Claude-managed global instruction source instead of maintaining a second authored copy

## Using Claude Code Commands

In Claude Code, type `/command_name` to invoke. These are the same commands available in OpenCode.

| Command | Purpose |
|---------|---------|
| `/design_doc` | Create a technical design document for a feature or system |
| `/create_plan` | Design a detailed implementation plan with iterative research |
| `/review_plan` | Validate a plan for architecture, bugs, and completeness |
| `/implement_plan` | Execute plan phases with verification |
| `/cleanup_plan` | Tear down completed plan worktrees and finish cleanup |
| `/research_codebase` | Document how the codebase works (read-only) |
| `/document_codebase` | Diagnose documentation gaps and fix them at the right level |

## Using Codex Skills

Codex skills are prompt-triggered, not slash commands. Use them by asking naturally or naming the skill explicitly:

| Skill | What it does | Example prompt |
|-------|-------------|----------------|
| `create-plan` | Build a phased implementation plan | "Use create-plan to plan the auth refactor" |
| `design-doc` | Draft a technical design document | "Use design-doc to design the new caching layer" |
| `document-codebase` | Audit and improve project documentation | "Use document-codebase to review docs under pde/" |
| `implement-plan` | Execute an approved plan with verification | "Use implement-plan on docs/PDEV-006.md" |
| `cleanup-plan` | Clean up a completed plan worktree safely | "Use cleanup-plan for the finished auth refactor worktree" |
| `research-codebase` | Explain how existing code works | "Use research-codebase to explain how install.sh works" |
| `review-plan` | Review a plan for architecture, bugs, completeness | "Use review-plan on docs/design-auth.md with focus on security" |

Skills are installed to `~/.codex/skills/`. Codex matches your prompt to the right skill automatically, and its global `AGENTS.md` is linked to `~/.claude/CLAUDE.md` during installation so the high-level workflow defaults stay aligned with Claude Code.

## Requirements

- Linux system with `sudo` access for PDE installs.
- `jq` is required for the Claude status line and is installed by `./install.sh` on Linux.
