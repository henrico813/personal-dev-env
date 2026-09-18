# Agents

Specialized task agents spawned by commands and reviewers to conduct parallel research.

## Codebase Agents

| Agent | Purpose |
|-------|---------|
| `codebase-locator` | Find files and directories relevant to a topic |
| `codebase-analyzer` | Understand how code works and trace data flow |
| `codebase-pattern-finder` | Find similar features and existing patterns |

## Documentation Agents

| Agent | Purpose |
|-------|---------|
| `docs-locator` | Discover what documents exist about a topic |
| `docs-analyzer` | Extract insights from specific documents |
| `docs-reviewer` | Review scoped work for stale, misleading, noisy, or missing documentation |
| `docs-writer` | Implement supported documentation findings without changing behavior |

## Plan Review Agents

| Agent | Purpose |
|-------|---------|
| `plan-architecture-reviewer` | Evaluate module design, complexity direction, information hiding |
| `plan-bug-reviewer` | Identify edge cases, error handling gaps, race conditions |
| `plan-completeness-reviewer` | Find missing files, integration points, tests |

## Research Agent

| Agent | Purpose |
|-------|---------|
| `web-search-researcher` | Find external documentation and resources (on request) |

## How They Work

Commands use these agents when their scope benefits from focused delegation:
- Each agent receives a focused prompt
- Independent agents may work concurrently
- Results are synthesized when all complete

`/create_plan` and `/review_plan` scale delegation to uncertainty and review
risk; bounded changes may be handled directly. `/implement_plan` delegates only
when an implementation step benefits from separate execution.

Most agents are read-only. The exception is `docs-writer`, which may modify
documentation within its delegated scope.
