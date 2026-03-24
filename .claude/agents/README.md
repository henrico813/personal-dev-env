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
| `docs-reviewer` | Analyze documentation for gaps and outdated content |
| `docs-writer` | Create and update README.md files based on review findings |

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

Commands like `/create_plan` and `/review_plan` spawn multiple agents in parallel:
- Each agent receives a focused prompt
- Agents work concurrently to maximize efficiency
- Results are synthesized when all complete

Agents are read-only and never modify files.
