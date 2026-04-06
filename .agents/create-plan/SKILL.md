---
name: create-plan
description: Use when the user asks to create a research-backed implementation plan and render it through the shared Go create-plan engine.
---

# Create Plan Engine

This package is the canonical source for the shared `create_plan` workflow.

## Workflow

1. Research the task and read all user-mentioned files fully before drafting.
2. Produce plan data as structured JSON, not freeform markdown.
3. Render that JSON through the installed Go helper for the active tool.
4. Write the rendered issue to the requested destination, usually the vault.

## JSON Contract

Produce a JSON object matching this shape:

```json
{
  "title": "Short title for the plan",
  "overview": "2-4 sentence summary of what the plan changes and why.",
  "definition_of_done": {
    "narrative": "Paragraph describing why the change matters and how the pieces fit together.",
    "goals": [
      "Concrete acceptance criterion"
    ],
    "current_state": "Current behavior, constraints, and relevant file:line references.",
    "module_shape": "Target file and directory structure after the change."
  },
  "implementation": [
    {
      "title": "Short step title",
      "summary": "Narrative summary explaining what this step changes and why.",
      "file_changes": [
        {
          "filename": "path/to/file.ext",
          "explanation": "One sentence explaining why this code exists.",
          "language": "text",
          "code": "representative changed code"
        }
      ]
    }
  ],
  "verification": {
    "summary": "Optional summary describing how verification maps to the goals.",
    "automated": [
      "Runnable automated check"
    ],
    "manual": [
      "Manual verification step"
    ]
  }
}
```

## Validation Rules

The engine rejects plans that do not satisfy this contract:

- non-empty `title`, `overview`, and `definition_of_done.narrative`
- at least one `definition_of_done.goals` item
- non-empty `definition_of_done.current_state` and `definition_of_done.module_shape`
- at least one implementation step
- every implementation step has non-empty `title`, `summary`, and at least one `file_change`
- every `file_change` has non-empty `filename`, `explanation`, `language`, and `code`
- rendered output must contain `Overview`, `Definition of Done`, `Current State`, `Module Shape`, `Implementation`, and `Verification`
- every implementation step must render at least one fenced code block

## Installed Helpers

Use the installed helper for the active tool:

- Claude: `~/.claude/bin/create_plan <plan.json> <output.md>`
- OpenCode: `~/.config/opencode/bin/create_plan <plan.json> <output.md>`
- Codex: `bin/create-plan <plan.json> <output.md>`

Do not emit freeform markdown directly when the installed helper is available.
