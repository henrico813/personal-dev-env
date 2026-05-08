# surveil

`surveil` works with structured task documents.

## Task format

```md
# Task

## Summary
- short summary text

## Explicit Files
- path/to/file.rs

## Search Areas
- src/
- docs/

## Questions
- What changed?
- What still needs verification?

## Terms
- optional keywords
```

Notes:
- `# Task` is the title and is ignored by the parser.
- Only the `##` sections above are interpreted.
- `## Questions` is required.
- `## Terms` is optional.

## Commands

- `surveil gather --repo <repo> --task-file <task.md>` emits a `GatherOutput` JSON context.
- `surveil research --context <context.json> --trace-out <trace.json>` emits a question-centered `ResearchOutput` JSON report and writes a shallow `TraceOutput` JSON file.

## Output shape

Research answers are grouped by question:
- `question`
- `findings` with `path`, `line`, `excerpt`, `source`, and `matched_from`
- `negative_evidence`
