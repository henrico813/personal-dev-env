# Hooks

Event-driven scripts that modify Claude Code behavior automatically.

## Hook List

| Hook | Trigger | Purpose |
|------|---------|---------|
| `clean_commit_guard.py` | PreToolUse | Removes Claude/Anthropic references and emojis from commits |
| `emoji_remover.py` | PostToolUse | Blocks files containing emoji characters |
| `github_issue_guard.py` | PreToolUse | Prevents Claude mentions in GitHub issues |
| `protect_claude_md.py` | PreToolUse | Blocks modifications to user-level CLAUDE.md |
| `docs_helper.py` | UserPromptSubmit | Auto-injects docs context when you ask help questions |
| `session_sync.py` | SessionStart | Syncs documentation if >24h stale or missing |

## How Hooks Work

Hooks are Python scripts that:
1. Read JSON from stdin with tool/command information
2. Check for policy violations
3. Exit with code 2 to block, 0 to allow
4. Use stderr for error messages

Most hooks silent-fail to avoid breaking workflows.

## Example: Commit Guard

Blocks:
```bash
git commit -m "Generated with Claude"
git commit --author="Claude <claude@anthropic.com>"
```

Suggests cleaning the command.

## Example: Docs Helper

When you ask:
```
How do I use hooks in Claude Code?
```

Automatically searches `.claude/docs/` and your config, injects relevant excerpts.
