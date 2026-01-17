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
| `/setup` | Configure docs structure for current project |
| `/create_plan` | Create implementation plans |
| `/research_codebase` | Document codebase state |
| `/review_plan` | Validate a plan |
| `/implement_plan` | Execute a plan |

## What install.sh Preserves

The install script preserves your user data across updates:

| File | Contents |
|------|----------|
| `projects/` | Session conversation data |
| `.credentials.json` | Authentication credentials |
| `history.jsonl` | Prompt history (up-arrow recall) |

Old configurations are backed up to `~/.claude.backup.<timestamp>`.

## Requirements

- `jq` (optional, for status line): `sudo apt install jq`
