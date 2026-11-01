# Personal Dev Environment

Shell, editor, and AI tooling configuration. **`pde-installer`** installs
machine, config, Obsidian, and AI tooling.

## PDE Quick Start

Build the installer from an existing repository checkout, then reconcile the
pinned environment:

```bash
mkdir -p ~/.local/bin
go build -C pde-installer -o ~/.local/bin/pde-installer .
export PATH="$HOME/.local/bin:$PATH"
pde-installer install terminal
```

Building the installer needs Go 1.21 or newer, for example from
`sudo apt install golang-go` on Ubuntu 24.04.

`terminal` is for headless homelab VMs, `client` for a laptop that edits files
and shows Herdr sessions from other machines, with nothing compiled by the
installer, and `full` for development machines. A fresh installation must choose
`terminal`, `client`, or `full`; the installer saves that selection in
`~/.config/pde/config.json`. Later bare `install` runs reuse the saved
selection, including the inferred full selection for legacy installer state. A
successful explicit selection switches future installs. A pre-commit failure
preserves the prior selection. Switching from full to terminal stops reconciling
full-only artifacts but does not uninstall them.

The independent `--color-profile` setting accepts `tokyo-night`,
`everforest-dark`, or `gruvbox-dark`. It defaults to Tokyo Night, is saved per
machine, and coordinates terminal, tmux, shell, and client or full Neovim and
OpenCode configuration included by the selected component profile.

The installer does not clone or update the repository. Run it from anywhere
inside the checkout, pass `--repo-root /path/to/personal-dev-env`, or set
`PDE_REPO_ROOT`.

The installer exposes three commands:

```bash
pde-installer install
pde-installer doctor
pde-installer list
```

- `install [terminal|client|full]` reconciles tools and managed home
  configuration in dependency order. A fresh installation requires an explicit
  selection; later bare installs reuse saved or legacy selection.
- `doctor` validates host prerequisites, pins, and managed paths for the saved
  profile (or full on a fresh `HOME`).
- `list` reports ownership and installed status for the saved profile (or full
  on a fresh `HOME`).
Run `pde-installer install --help` before maintaining an existing installation.
See the
[command reference](pde-installer/docs/reference/commands.md) for details.

Mutating commands reject UID 0. The installer uses `sudo apt-get` for missing
Ubuntu dependencies. Other managed files stay below `HOME`. Use `--dry-run` to
preview ordered work without making changes.

The installer saves which checkout it installed from in
`~/.config/pde/config.json`. To tell Neovim and nobs where your Obsidian
vaults are, run `nobs vault set NAME PATH` once for each vault, for
example `nobs vault set main ~/Documents/main-vault`.

See [`pde-installer/README.md`](./pde-installer/README.md) for installer details.

## AI Tools Quick Start

```bash
pde-installer install full
```

The full install includes the AI tooling. It installs planner, Codex,
OpenCode, the OpenCode inline shim, Pi, Moshi Hook, Herdr, nobs, and
Vibe binaries plus repo-managed AI config.

## AI Source Tree

- `ai/AGENTS.md` is the shared workflow default file.
- `ai/skills/` holds shared Agent Skills-format guidance, including the shared
  workflow skills.
- `ai/opencode/` holds OpenCode agents and commands.
- `ai/pi/agent/` holds Pi settings and package resources.
- `pde/AGENTS.md` holds repo-local PDE notes.
- `nvim-plugins/` holds custom Neovim plugins that also install outside PDE.

## Installed Layout

| Tool | Config source | Install target | Invocation style |
|------|--------------|----------------|-----------------|
| planner | `planner/` | `~/.local/bin/planner` | Shared plan CLI |
| Vibe | `vibe/` | `~/.local/bin/vibe` | Worktree-backed execution harness |
| Create plan | `ai/skills/create-plan/` | `~/.agents/skills/create-plan/` | Shared workflow skill for implementation proposals with exact code diffs |
| Review plan | `ai/skills/review-plan/` | `~/.agents/skills/review-plan/` | Shared workflow skill for reviewing plans and code changes |
| Implement plan | `ai/skills/implement-plan/` | `~/.agents/skills/implement-plan/` | Shared workflow skill for stepwise plan execution |
| Cleanup plan | `ai/skills/cleanup-plan/` | `~/.agents/skills/cleanup-plan/` | Shared workflow skill for plan teardown |
| Design doc | `ai/skills/design-doc/` | `~/.agents/skills/design-doc/` | Shared workflow skill for technical design documents |
| Research codebase | `ai/skills/research-codebase/` | `~/.agents/skills/research-codebase/` | Shared workflow skill for as-is codebase research |
| Delegate | `ai/skills/delegate/` | `~/.agents/skills/delegate/` | Shared workflow skill for handing a task to an agent and reviewing it |
| Herdr handoff | `ai/skills/herdr-handoff/` | `~/.agents/skills/herdr-handoff/` | Shared workflow skill for handing work to a new session in a new Herdr workspace |
| Roadmap run | `ai/skills/roadmap-run/` | `~/.agents/skills/roadmap-run/` | Shared workflow skill for building a roadmap's pull requests as a stack while the user is away |
| Behavior-focused testing | `ai/skills/behavior-focused-testing/` | `~/.agents/skills/behavior-focused-testing/` | Shared test-writing guidance |
| Code documentation | `ai/skills/code-documentation/` | `~/.agents/skills/code-documentation/` | Proportional source and test explanations |
| Plain direct English | `ai/skills/plain-direct-english/` | `~/.agents/skills/plain-direct-english/` | Clear sentences in comments, docs and other prose, and breakdowns of hard ones |
| Status updates | `ai/skills/status-updates/` | `~/.agents/skills/status-updates/` | Status updates and recaps a new reader understands on a phone |
| Skill lessons skill | `ai/skills/skill-lessons/` | `~/.agents/skills/skill-lessons/` | How to follow and save the lessons in a skill's Lessons section |
| Comment review skill | `ai/skills/comment-review/` | `~/.agents/skills/comment-review/` | Saves code comments as a review you answer one per screen |
| Go development | `ai/skills/go-development/` | `~/.agents/skills/go-development/` | Shared Go development guidance |
| Python development | `ai/skills/python-development/` | `~/.agents/skills/python-development/` | Shared Python development guidance |
| Rust development | `ai/skills/rust-development/` | `~/.agents/skills/rust-development/` | Shared Rust development guidance |
| TypeScript development | `ai/skills/typescript-development/` | `~/.agents/skills/typescript-development/` | Shared JavaScript and TypeScript development guidance |
| Pull request | `ai/skills/pull-request/` | `~/.agents/skills/pull-request/` | Pull-request procedure |
| Git messages | `ai/skills/git-messages/` | `~/.agents/skills/git-messages/` | Shared commit and PR guidance |
| Promote memory | `ai/skills/promote-memory/` | `~/.agents/skills/promote-memory/` | Reviewed memory-to-skill promotion |
| Obsidian Zettel | `ai/skills/obsidian-zettel/` | `~/.agents/skills/obsidian-zettel/` | Template-aligned vault note creation |
| Obsidian daily note | `ai/skills/obsidian-daily-note/` | `~/.agents/skills/obsidian-daily-note/` | Approved sections appended to today's daily note |
| OpenCode | `ai/opencode/`, `chezmoi/` | `~/.config/opencode/{agents,commands,cli.json,pde-herdr-opencode}`, `opencode.json` permission merge | OpenCode commands, agents, synchronized TUI theme, bidirectional agent cycling (`Tab` next, `Shift+Tab` previous), and a Herdr pane that stays working while background subagents run |
| Herdr layout | `chezmoi/dot_config/herdr/` | `~/.config/herdr/config.toml` (client and full) | Mobile-first agent workspace UI; Herdr is the installer-managed Herdr release |
| OpenCode memory | `ai/AGENTS.md`, `chezmoi/` | Existing `~/.opencode-mem/` data | Unsupported with the managed Claude adapter |
| OpenCode Inline | `nvim-plugins/opencode-inline.nvim/` | `~/.config/nvim/pack/plugins/start/opencode-inline.nvim`, `~/.local/bin/opencode-inline-shim` | CodeCompanion inline plugin and OpenAI-compatible bridge |
| Pi | `ai/pi/agent/` | `~/.local/bin/pi`, `~/.pi/agent/` | Managed CLI plus settings |
| Moshi Hook | `pde-installer/internal/direct/` | `~/.local/bin/moshi-hook`, `~/.local/bin/moshi` | Mobile coding-agent hook daemon |
| Herdr | `pde-installer/internal/direct/` | `~/.local/bin/herdr` | Agent-aware terminal workspace |
| nobs | `pde-installer/internal/direct/` | `~/.local/bin/nobs` | Finds, reads and syncs vault notes; run `nobs vault set` once per vault |
| PR write approval | `pdev2/`, `chezmoi/` | `~/.local/bin/{pde-gh-write,pde-pr-approve,gh}`, `~/.local/share/pde/herdr-plugins/pde-approval/` (full only) | Agents write to pull requests only after you approve; see the [approval docs](pdev2/docs/README.md) |
| Comment review | `pde-comment-review/`, `chezmoi/` | `~/.local/bin/pde-comment-review`, `~/.local/share/pde/herdr-plugins/pde-comment-review/` (full only) | Decide on an AI's comments one per phone screen with Ctrl+b e; see the [comment review docs](pde-comment-review/docs/README.md) |
| Skill lessons | `skill-lessons/` | `~/.local/bin/skill-lessons` (client and full) | Saves your corrections as lessons in the installed copy of a skill; see the [skill-lessons README](skill-lessons/README.md) |
| Comment checker | `pde-comment-checker/` | `~/.local/bin/pde-comment-checker` (full only) | Checks agents' code comments and keeps the ones you approved; see the [comment checker README](pde-comment-checker/README.md) |
| prs | `prs/` | `~/.local/bin/prs` (full only) | Reads your open pull requests on a phone; see [GitHub Pull Request Reviews](chezmoi/README.md#github-pull-request-reviews) |

Shared configuration lives in `chezmoi/`, including local-file mappings for the complete `ai/` source tree and checksummed remote externals.


PDE owns the installed Claude Code package and the OpenCode Claude adapter
configuration. Installation does not authenticate Claude Code; authenticate it
with `claude auth login --claudeai`. The supported pairing is:

- OpenCode: `@opencode/cli@2.0.26`,
- Claude adapter: `@openchamber/opencode-claude@1.1.0`,

The managed-plugin merge removes stale entries for managed Claude and memory
plugins, preserves unrelated plugins and settings, and appends the pinned
adapter. OpenCode memory is intentionally unsupported with this integration.
Existing `~/.opencode-mem/` data remains on disk, but the full installation
removes its plugin entry and OpenCode does not load it.

## Using OpenCode Commands

In OpenCode, type `/command_name` to invoke. These are the same commands
installed from `ai/opencode/commands/`. Each command is a thin wrapper that
loads the matching shared workflow skill from `ai/skills/`, so the workflow
instructions live once in the skill.

| Command | Purpose |
|---------|---------|
| `/design_doc` | Create a technical design document for a feature or system |
| `/create_plan` | Produce a repository-grounded implementation proposal |
| `/review_plan` | Review a plan, PR, commit, or diff for architecture, bugs, and completeness |
| `/implement_plan` | Execute plan phases with verification |
| `/cleanup_plan` | Clean completed plan, worktree, branch, PR evidence, and main state |
| `/research_codebase` | Document how the codebase works (read-only) |

## Using the Shared Workflow Skills

The shared workflow skills are prompt-triggered, not slash commands. Use them
by asking naturally or naming the skill explicitly, in any harness that
loads `~/.agents/skills/`.

| Skill | What it does | Example prompt |
|-------|-------------|----------------|
| `create-plan` | Create a repository-grounded implementation proposal | "Use create-plan to plan the auth refactor" |
| `design-doc` | Draft a technical design document | "Use design-doc to design the new caching layer" |
| `implement-plan` | Execute an approved plan with verification | "Use implement-plan on docs/PDEV-006.md" |
| `cleanup-plan` | Clean completed plan, worktree, branch, PR evidence, and main state | "Use cleanup-plan for the merged auth refactor branch" |
| `research-codebase` | Explain how existing code works | "Use research-codebase to explain how pde-installer install works" |
| `delegate` | Have an agent do a task, review and iterate until it is ready | "Have an agent fix the flaky test, review it until it's ready for me" |
| `herdr-handoff` | Hand work to a new session in a new Herdr workspace, with a handoff file it reads first | "Hand the docs work to a new session" |
| `roadmap-run` | Plan, build, review and file the remaining PRs of an approved roadmap as a stack | "Build the rest of the roadmap overnight" |
| `review-plan` | Review a plan, PR, commit, or diff for architecture, bugs, completeness | "Use review-plan on docs/design-auth.md with focus on security" |

The skills are installed to `~/.agents/skills/`, and the installer copies the
shared `AGENTS.md` into `~/.codex/` and `~/.claude/` so the workflow defaults
stay aligned with the rest of the tree. `install client` installs only
`obsidian-zettel`, `obsidian-daily-note`, `plain-direct-english`,
`behavior-focused-testing`, `delegate`, `skill-lessons` and
`status-updates`; every other skill is full-only.

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
go test -race -C pde-installer ./...
go vet -C pde-installer ./...
```

These tests protect the operations most likely to damage an existing setup:
path containment, package-state decisions, exact version checks, installer
locking, durable recovery, backend activation, and rollback after a later
failure.

Docker is still used to confirm that the command behaves as an unprivileged
process on supported Ubuntu releases:

```bash
./pde-installer/test/run-tests.sh smoke
```

The smoke runs all three commands as an unprivileged user on Ubuntu 22.04 and
24.04 and confirms that root execution is rejected. Go tests cover managed
configuration transactions and rollback. Install is a dry run in the full
smoke; the terminal suite performs a real installation.
