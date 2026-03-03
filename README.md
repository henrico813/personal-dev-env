# Personal Claude Config

Claude Code configuration with privacy-focused defaults and workflow commands.

## Quick Start

```bash
./install.sh
```

Then in any project:
```
/setup
```

## Commands

| Command | Purpose |
|---------|---------|
| `/setup` | Create flat docs/ directory in current project |
| `/create_plan` | Create detailed implementation plans through iterative research |
| `/research_codebase` | Document codebase architecture and behavior |
| `/review_plan` | Validate plans for architecture and completeness |
| `/implement_plan` | Execute an implementation plan with verification |
| `/document_codebase` | Auto-generate directory-local documentation |
| `/export_plan` | Export current plan to markdown for handoff |
| `/git_commit` | Create intelligent commits from conversation context |

## What install.sh Preserves

The install script preserves your user data across updates:

| File | Contents |
|------|----------|
| `projects/` | Session conversation data |
| `.credentials.json` | Authentication credentials |
| `history.jsonl` | Prompt history (up-arrow recall) |

Old configurations are backed up to `~/.claude.backup.<timestamp>`.

## Hooks

Auto-documentation and safety features:

| Hook | What It Does |
|------|--------------|
| **Commit Guard** | Removes Claude/Anthropic references from commits |
| **Emoji Remover** | Blocks files with emoji characters |
| **GitHub Guard** | Prevents Claude mentions in GitHub issues |
| **Docs Helper** | Auto-injects relevant documentation for help queries |
| **Session Sync** | Keeps documentation fresh (syncs if >24h stale) |

See [`.claude/hooks/README.md`](./.claude/hooks/README.md) for details.

## Requirements

- `jq` (for status line): `sudo apt install jq`

## Specstory

Specstory captures Claude Code sessions as Markdown files in `.specstory/history/`.

Launch Claude with session capture (saves all sessions to a central dir):

    specstory run --output-dir ~/.claude/specstory --no-cloud-sync claude -c "claude --dangerously-skip-permissions"

Sync past sessions from `~/.claude/projects/`:

    specstory sync claude

## OpenCode Compatibility

Commands and agents are also available in [OpenCode](https://opencode.ai) via `.opencode/` configs (stripped of Claude-specific frontmatter fields). Run `install.sh` to copy them to `~/.config/opencode/`.
