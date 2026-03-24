# Claude Code Configuration

Personal Claude Code settings with privacy-focused defaults and workflow automation.

## Directory Structure

- `agents/` - Task agents for specialized research (codebase analysis, documentation review, plan evaluation)
- `commands/` - Slash commands for common workflows (/create_plan, /implement_plan, etc.)
- `hooks/` - Event-driven scripts that modify Claude Code behavior
- `scripts/` - Utility scripts for hooks and automation
- `CLAUDE.md` - Core configuration with global instructions

## How It Works

### Commands
The `/` commands enable workflows:
- **Planning**: `/create_plan`, `/review_plan`, `/implement_plan`
- **Research**: `/research_codebase`, `/document_codebase`

### Hooks
Hooks execute automatically on events:
- **Commit Guard**: Removes Claude/Anthropic references from commits and blocks emojis
- **GitHub Guard**: Prevents mentions in GitHub issues
- **CLAUDE.md Protection**: Blocks modifications to user-level CLAUDE.md
- **Emoji Remover**: Blocks files with emojis (PostToolUse)
- **Docs Helper**: Auto-injects documentation context on help queries (UserPromptSubmit)
- **Session Sync**: Syncs docs on startup if >24h stale (SessionStart)

### Agents
Specialized agents for parallel research:
- **Codebase agents**: locator, analyzer, pattern-finder
- **Docs agents**: locator, analyzer, reviewer, writer
- **Plan agents**: architecture-reviewer, bug-reviewer, completeness-reviewer

## Key Features

- **Privacy-focused**: Removes Claude attribution from commits automatically
- **Auto-documentation**: Helps provide relevant docs when you ask questions
- **Plan workflow**: Create, review, and implement detailed technical plans
- **Context preservation**: Syncs docs hourly, tracks session state

## Configuration

Edit `.claude/CLAUDE.md` to change core behaviors (commit authorship rules, code comment style, etc.).
