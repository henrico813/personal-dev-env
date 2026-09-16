---
description: Create detailed implementation plans through the shared Go planner CLI
---

# Create Plan

You are tasked with creating detailed implementation plans that are grounded in the actual codebase and ready for execution.

## Task Context

Treat the following command arguments as the user's task context. An empty
value means no task was provided.

$ARGUMENTS

Your default behavior is:

1. Read all provided context fully.
2. Load every available skill applicable to work identified in the request or
   referenced context.
3. For repo-backed implementation planning, use surveil as the default research engine.
4. Resolve uncertainty through investigation whenever possible.
5. Produce the full plan including diffs of all lines needed for a code change.
6. Track the ordered list of skills successfully loaded while planning.

Ask the user clarifying questions only when missing information would materially change the implementation, sequencing, or verification. Do not ask for approval on plan structure or phasing. Planner owns the document structure.
  
## Initial Response

When this command is invoked:
  
1. **If parameters were provided**:
   - If a file path, ticket reference, or document path was provided, read it fully.
   - Begin research immediately.

2. **If no parameters were provided**:
```text
I'll help you create a detailed implementation plan.

Please provide:
1. The task or ticket description, or a path to the design doc / issue
2. Any constraints or requirements that materially affect implementation
3. Links or paths to related docs, previous plans, or prior implementations

I'll research the relevant code and produce a concrete implementation plan.

Tip: You can invoke this command with a file directly: `/create_plan docs/design-feature-name.md`
```

Then stop and wait for the user to provide the task.

## Non-Negotiable Rules

- Read supplied and directly referenced context before the initial skill check.
- Load matching domain skills before Surveil setup, research agents, or manual
  repository research.
- If research identifies another affected domain, load its skill and revisit
  decisions made without it.
- Read every mentioned file fully before drafting the plan.
- Research the relevant code, tests, config, and documentation before drafting the plan.
- For repo-backed implementation plans, treat `surveil` artifacts as required baseline inputs before broad manual repo research.
- Do not draft the final plan until research is complete.
- If blocking questions remain after research, ask only those questions and stop.
- Determine the final output path before running `planner new "<output.md>"` or scaffolding `surveil`.
- Classify the operation as a new plan or partial update. For a new plan, verify
  the destination is absent before research and again immediately before
  `planner new`; if it exists, stop and ask whether to update it or use another
  path. Never overwrite an existing plan implicitly.
- Write the final plan to its real destination, not a transient temp path.
- If the user says a legacy surface is being phased out, treat that surface as out of scope unless they explicitly request changes there.
- Use the unique managed root printed by `surveil new task --task architecture`; do not create or reuse a shared task path.
- Avoid dependency, virtualenv, worktree, generated output, and cache directories unless explicitly requested.
- Use exactly the required headings and heading order in the final plan unless the user explicitly asks for a different format.
- Do not add extra sections unless the user explicitly asks for them.
- Keep the final plan actionable. The output is an implementation issue, not a design brainstorm.
- Prefer `make` commands in verification when suitable targets exist. If no suitable `make` target exists, say so and use the direct command.
- Exact code in diff blocks must be provided for all implementation and verification steps. Do not omit any lines of code or commands. This is a requirement for the plan to be actionable, reviewable, and unambiguous.
- Maintain `<loaded-skills>` as the ordered, de-duplicated names of skills the
  parent successfully loads, including skills discovered later.
- If an applicable skill cannot be loaded, report it and stop before research;
  do not silently omit it from `<loaded-skills>`.
- Reconcile material evidence before drafting; an unresolved material finding
  blocks plan creation.
- Review a completed plan against `<loaded-skills>` with an independent agent;
  do not substitute a parent-only self-check.

## Workflow

First classify whether the request is a repo-backed implementation plan.

Treat the request as repo-backed and use `surveil` when any of these are true:
- the user wants an implementation plan for an existing repo feature, refactor, bug fix, or integration
- the plan depends on current code, tests, config, docs, or workflow behavior
- the request names files, modules, services, commands, or directories in this repo

Skip `surveil` when:
- the request is purely conceptual or comparative
- the user is brainstorming options without needing concrete repo research
- there is no meaningful local codebase surface to inspect

If any repo-backed trigger is present, do not fall back to manual-first research.

### Step 1: Read and Gather Context

1. Read all files mentioned by the user fully.
2. Read any directly related design docs, research docs, prior implementation plans, and referenced JSON or data files fully.
3. Resolve plan, issue, doc, and vault references using the Planning Docs guidance in `ai/AGENTS.md`.
   - Use existing filesystem paths directly.
   - For existing plans, docs, and notes, use `pde vault locate --json --vault <selector> "<reference>"`.
   - Use `pde vault path <selector>` only when determining the destination root for a new plan or when the user explicitly asks for a vault root.
   - Ask only on `ambiguous`, `not_found`, or setup `error`.
4. Identify the code paths, modules, tests, config, and docs that are likely to be affected.
5. Determine the exact output path, classify the operation as a new plan or
   partial update, and apply the destination-existence rule above.

### Step 2: Research the Codebase

For repo-backed implementation plans, use `surveil` as the default research workflow.

For requests that are not repo-backed, skip `surveil`. Run `mktemp -d
"${TMPDIR:-/tmp}/create-plan-research.XXXXXX"` once and capture its exact
output as `<research-artifact-dir>`, then research the relevant code, tests,
config, docs, or comparative material directly using the available read-only
tools before drafting.

For this workflow, <task-context> is the user's request and every referenced document.

For this workflow, <evidence-review-agent> is an independent OpenCode `general`
agent restricted by its delegation prompt to read-only research.

For repo-backed planning, run `planner help` and `planner workflow help` before
the first Planner command. Run `planner workflow id`, capture its
`workflow_id`, then run `planner workflow start --id <workflow-id> --repo
"<repo>" --output "<output.md>" --operation new` with one `--skill <name>` for
each loaded skill. For a partial update, also pass `--input "<input.md>"` and
use `--operation partial-update`; input and output may name the same file.
Capture the returned revision. Store the workflow ID and exact `planner
workflow show <workflow-id>` command in TodoWrite. Run that read-only command
after compaction or handoff and before each delegation, drafting step, or
review; its strict JSON is the lifecycle state. Never read or edit Planner's
private state files directly.

Generate the ID before start so a failed stdout write is recoverable. After
any state-changing Planner command exits nonzero, run `planner workflow show
<workflow-id>`. Continue only when the returned stage and revision prove the
requested transition committed; otherwise use the returned revision to record
a terminal failure when possible and stop. Never retry a transition blindly.

For conceptual planning, use the temporary `<research-artifact-dir>` created
above and write `<research-artifact-dir>/planning-state.md` with the operation,
output identity, ordered skills, referenced context, and completed evidence.
Update and reread it after handoffs and before delegation, drafting, or review.
Store its path in TodoWrite. Do not invoke `planner workflow` or Surveil for
that branch.

For either independent reviewer, apply this read-only guard around each agent
attempt:

1. Capture the output path's existence or hash, hashes of every supplied
   artifact and directly referenced file outside the repo, the partial-update
   input plan when distinct from the output, and absence of the intended review
   artifact.
2. For a Git-backed repo, also capture a content fingerprint of tracked and
   staged changes plus non-ignored untracked files, and capture the current
   revision. Use `git rev-parse HEAD`, `git diff --binary HEAD`, and
   `git ls-files --others --exclude-standard` as the inventory sources. For a
   Git repo with an unborn branch, capture `git symbolic-ref HEAD`,
   `git diff --binary --cached`, and `git diff --binary` instead. For a non-Git
   repo, fingerprint all non-excluded source files.
3. Immediately after the agent returns or fails, before using or saving its
   response, compare every captured value. Any change is an unauthorized
   mutation: stop, report the changed path or repo state, and do not use the
   review response.

## Managed Repo Research

1. Run `surveil new task --task architecture` and capture its exact output as
   `<search-dir>` and `<research-artifact-dir>`. If root creation fails, run
   `planner workflow fail <workflow-id> --expected-revision <N> --stage
   research --reason "<concise error>"` and stop.
2. Bind that absolute root with `planner workflow research bind <workflow-id>
   --expected-revision <N> --managed-root "<search-dir>"`. Replace `<N>` with
   every returned revision. Create `interfaces-data-state` and
   `tests-verification` with `surveil new task --root "<search-dir>" --task
   <name>`. Any setup failure is terminal and must be recorded with `workflow
   fail --stage research`.
3. Populate `<search-dir>/architecture/task.json`,
   `<search-dir>/interfaces-data-state/task.json`, and
   `<search-dir>/tests-verification/task.json` from <task-context>:
   - Set `summary` to the task-context title; if it has no title, use its first sentence verbatim.
   - Set `explicit_files` to only literal paths named by the task context, preserving first-seen order and removing exact duplicates.
   - Set `search_areas` to the smallest repo directories covering those paths and each task's focus; use `.` only when the intended scope is the repository root.
   - Treat every relative `explicit_files` and `search_areas` value as relative to the exact <repo> passed to `--repo`; recalculate them if <repo> changes.
   - Set `terms` to literal identifiers, filenames, path segments, commands, and feature names, de-duplicate case-insensitively, and do not invent synonyms.
4. Populate each task's `query` array with its ordered questions. Do not omit, reorder, combine, reword, or reuse question sets across tasks.
   - `architecture`:
     1. `How does the current command or request flow through this area?`
     2. `Which modules own this behavior, and where are their boundaries?`
     3. `Which callers and integration points would need to change?`
     4. `What orchestration or dependency direction must be preserved?`
     5. `Which files define the complete implementation path?`
   - `interfaces-data-state`:
     1. `Which structs, types, functions, and fields define this behavior?`
     2. `How does data enter, change, and leave this area?`
     3. `Which validation rules and invariants must be preserved?`
     4. `Which persistence, environment, filesystem, process, or API boundaries are involved?`
     5. `Which compatibility or migration concerns apply?`
   - `tests-verification`:
     1. `Which existing tests and fixtures cover this behavior?`
     2. `Which test helpers and patterns should new coverage follow?`
     3. `Which docs, config, commands, and CI targets affect this change?`
     4. `Which automated checks verify the implementation?`
     5. `Which behavior requires manual verification?`
5. Run `surveil index --repo "<repo>"`. If indexing fails, record `workflow
   fail --stage research` and stop.
6. Run `surveil session run --repo "<repo>" --root "<search-dir>"` once. The
   authoritative receipt is
   `<search-dir>/.surveil-session/receipt.json`. After a nonzero exit, inspect
   only that path: continue when it exists because publication completed before
   stdout failed; otherwise record `workflow fail --stage research` and stop.
   Do not retry the completed root and do not produce fallback evidence.
7. Run `planner workflow research complete <workflow-id>
   --expected-revision <N> --receipt "<receipt-path>"`. Planner strictly
   validates the PDEV-161 receipt and every listed artifact. Set
   `<evidence-path>` to `<search-dir>/.surveil-session/evidence.json`.
8. Read `<evidence-path>` before additional repository research. If a read is
   truncated, use targeted searches and direct file reads rather than treating
   partial output as complete evidence.
9. After successful evidence, run one <evidence-review-agent>:
    - Apply the read-only guard to `<output.md>`, the repo, all supplied
      artifacts, and `<research-artifact-dir>/manual-review.md`.
    - Name each applicable skill in the delegation prompt.
    - Require the agent to load available applicable skills before review and
      report any required skill that is unavailable.
    - Give it <task-context>, <repo>, `<receipt-path>`, `<evidence-path>`, and
      the current `planner workflow show <workflow-id>` result.
    - Find required files or behavior missing from the evidence and correct assumptions not supported by direct file reads.
    - Check related callers, integration points, and existing patterns outside the searched areas.
    - Identify missing tests, fixtures, config, commands, CI checks, or manual verification.
    - Require read-only research with concrete `file:line` references and findings not already present in the evidence. Prohibit edits and mutating commands.
    - Save its final response verbatim as `<research-artifact-dir>/manual-review.md`.
    - If the agent invocation fails, retry it once with a fresh read-only guard.
      If the artifact write fails, retry that write once. If either still
      fails, record `workflow fail --stage evidence` and stop.
10. Verify new or conflicting findings from <evidence-review-agent> with direct file reads before continuing.
11. Use `planner workflow skill add <workflow-id> --expected-revision <N>
    --name <skill>` immediately for every applicable skill discovered during
    research. All skill additions must finish before evidence completion.
## Evidence Review Best Practices

Run one research task after reading merged evidence. Give the analyzer exact directories, require read-only tools and `file:line` references, wait for it to complete, and cross-check unexpected findings directly.

Example:
```python
task = Task("Review Surveil evidence", evidence_review_prompt)
```

Assistant: This is a repo-backed implementation plan, so I'll create three managed Surveil tasks, merge their reports, and use one <evidence-review-agent> to review the evidence before drafting.

### Step 3: Reconcile Evidence

Before drafting, write `<research-artifact-dir>/evidence-disposition.md`.

1. Include only material findings from the supplied context, loaded skills,
   Surveil evidence, manual review, and direct verification.
2. For each finding, record its source, the finding, one disposition, and the
   concrete plan impact or exclusion reason.
3. Use only these dispositions:
   - `applied`: the plan will reflect the finding in a named decision, step, or verification item.
   - `no-plan-impact`: the finding is verified but immaterial or out of scope, with a concrete reason.
   - `unresolved`: evidence is insufficient or conflicting and the plan cannot safely choose an implementation.
4. If there are no material findings, write `No material findings.`
5. If any material finding is `unresolved`, ask the minimum blocking question
   when the user can answer it; otherwise report the evidence gap and stop. Do
   not create or finalize the plan.
6. Update the disposition if later research or a late skill load changes a
   material decision.
7. For repo-backed planning, after all skills and dispositions are final, run
   `planner workflow evidence complete <workflow-id> --expected-revision <N>
   --review "<research-artifact-dir>/manual-review.md" --disposition
   "<research-artifact-dir>/evidence-disposition.md"`. A skill discovered after
   this transition makes the lifecycle terminal: record `workflow fail --stage
   evidence` and stop rather than reviewing stale inputs.


### Step 4: Plan Structure Development

Once aligned on approach:

1. **Create the initial outline internally unless the user explicitly asked for interactive planning**:
   ```
   Here's my proposed plan structure:

   ## Overview
   [1-2 sentence summary]

   ## Implementation Phases:
   1. [Phase name] - [what it accomplishes]
   2. [Phase name] - [what it accomplishes]
   3. [Phase name] - [what it accomplishes]

   Use the smallest scope that satisfies the request and constraints.
   ```

### Step 5: Detailed Plan Writing

After research is complete:

1. Run `planner help` before the first Planner command and again if compaction
   obscures command shapes; do not guess them from memory.
2. For new plans, verify `<output.md>` is still absent immediately before
   running `planner new "<output.md>"`; stop instead of overwriting it if it
   now exists.
3. For partial updates, run `planner inspect "<plan.md>"` to see the parsed plan JSON and `update_diff_expect` tokens.
4. Prefer `planner patch "<plan.md>" "<out.md>"` for transactional scalar, checklist, and `Update Diff` edits; omit the second path for same-file patches.
5. Use behavioral commands when the patch v1 surface does not cover the change; for same-file behavioral edits, `<out.md>` may equal `<plan.md>`.
6. Preserve the supported wrapped-issue frontmatter on same-path edits.
7. Run `planner check "<output.md>" --json-errors` to establish a structurally valid draft before completed-plan review. Fix all reported failures together and rerun, for at most three correction rounds. If validation still fails, stop before review.

Do not emit freeform markdown directly when the installed helper is available.

Planner owns separators and formatting. The required heading order is:

1. `# <title>`
2. `## Overview`
3. `## Definition of Done`
4. `### Goals`
5. `### Current State`
6. `### Module Shape`
7. `## Implementation`
8. `### <number>. <step title>` for each implementation step
9. `## Verification`
10. `### Automated Verification`
11. `### Manual Verification`

#### Partial Updates

For targeted updates to an existing plan:

1. Run `planner inspect "<plan.md>"`
2. Prefer `planner patch "<plan.md>" "<out.md>"` for transactional scalar and checklist edits; omit the second path for same-file patches.
3. Fall back to behavioral commands such as `planner implementation step
   file-change add "<plan.md>" "<out.md>" --step N --filename F --explanation
   E --diff-stdin` when patch v1 does not cover the change.

Non-targeted sections are preserved byte-for-byte, including supported wrapped issue frontmatter.

### Step 6: Review Against Loaded Skills

If `<loaded-skills>` is empty, skip this step. Otherwise run one independent
OpenCode `general` agent after the draft passes its initial Planner check.

1. Apply the read-only guard to `<output.md>`, the repo, all supplied artifacts,
   and `<research-artifact-dir>/loaded-skill-review.md`.
2. Begin the delegation prompt with `Applicable skills: <loaded-skills>` using
   every exact skill name in order.
3. Require the reviewer to load every listed skill before reviewing and report
   any unavailable skill as a blocking finding.
4. Give it <task-context>, <repo>, <output.md>, the current `planner workflow
   show <workflow-id>` result,
   `<research-artifact-dir>/evidence-disposition.md`, and all available
   evidence, manual-review, and prior loaded-skill-review artifact paths.
5. Require read-only tools, prohibit edits and mutating commands, and require it
   to read the completed plan fully and review only whether the plan
   preserves the loaded skills' material guidance. It may identify unsupported
   behavior or missing verification when that demonstrates a skill violation,
   but it must not repeat a general architecture review.
6. Require concrete findings with severity, skill name, plan or source
   reference, reason, and required correction. Require an explicit `No
   findings.` result when none exist.
7. Immediately after the reviewer returns, apply the guard before saving its
   response, classifying findings, or changing the plan.
8. Save the final response verbatim as
   `<research-artifact-dir>/loaded-skill-review.md`.
   For repo-backed planning, classify the saved response as `pass` or
   `blocking`, then run `planner workflow review complete <workflow-id>
   --expected-revision <N> --path
   "<research-artifact-dir>/loaded-skill-review.md" --outcome <outcome>`.
9. If the reviewer invocation fails, retry once with a fresh read-only guard.
   For repo-backed planning, record `workflow fail --stage review` before
   stopping after the permitted retry or artifact-write failure.
   If saving the review artifact fails, retry the write once. Stop if either
   action still fails. Apply the same rules to the follow-up.
10. Treat unavailable required skills, contradicted skill guidance, unsupported
   behavior prohibited by a loaded skill, and omitted required verification as
   blocking.
11. If there are blocking findings, correct the plan. For conceptual planning,
   also correct the evidence disposition. Repo-backed evidence is frozen after
   `evidence complete`; if a finding requires changing it, record `workflow
   fail --stage review` and stop instead of recording a stale follow-up.
   Otherwise, rerun the initial Planner check, then run one independent
   follow-up with the same inputs and save it as
   `<research-artifact-dir>/loaded-skill-review-follow-up.md`.
   Record the follow-up's actual `pass` or `blocking` outcome with the same
   `planner workflow review complete` command. A second blocking result moves
   the lifecycle to terminal failure.
12. Apply a fresh read-only guard to the follow-up and its intended artifact.
    Any plan change after the initial review consumes this single correction
    cycle and requires the follow-up.
13. If the follow-up reports a blocking finding, either review agent fails
    after one retry, or either reviewer changes the plan, stop without reporting
    a completed plan. Do not replace the independent review with a self-review.

### Step 7: Validate And Report

1. For repo-backed planning, run `planner workflow finish <workflow-id>
   --expected-revision <N>`. Planner reruns plan validation and rechecks the
   receipt, receipt-listed artifacts, evidence review, disposition, every plan
   review, the latest reviewed plan bytes, and a distinct partial-update input.
2. For conceptual planning, run `planner check "<output.md>" --json-errors`.
   If final validation fails, stop; do not modify a reviewed plan without the
   permitted independent follow-up.
3. Report the final output path, completion identity or validation result,
   current workflow state or `<research-artifact-dir>/planning-state.md`,
   `<research-artifact-dir>/evidence-disposition.md`, the applicable
   loaded-skill review path, and any blockers.

## Important Guidelines

1. **Be Skeptical**:
   - Question vague requirements
   - Identify potential issues early
   - Ask "why" and "what about"
   - Don't assume - verify with code

2. **Be Directed**:
   - Begin research immediately when enough information is present to resolve inputs
   - Ask clarifying questions only when missing information would materially change implementation, sequencing, or verification
   - Do not pause for approval on plan structure or intermediate research updates

3. **Be Thorough**:
   - Read all context files COMPLETELY before planning
   - For repo-backed planning, use `surveil` before broad manual repo research
   - Research actual code patterns using parallel sub-tasks only after the initial `surveil` pass when additional investigation is still needed
   - Include specific file paths and line numbers
   - Write measurable success criteria with clear automated vs manual distinction
   - automated steps should use `make` whenever possible - for example `make -C myapp check` instead of `cd myapp && npm run fmt`

4. **Be Practical**:
   - Focus on incremental, testable changes
   - Consider migration and rollback
   - Think about edge cases
   - Include "what we're NOT doing"

5. **Track Progress**:
   - Use TodoWrite to track planning tasks
   - Update todos as you complete research
   - Mark planning tasks complete when done

6. **No Open Questions in Final Plan**:
   - If you encounter open questions during planning, STOP
   - Research or ask for clarification immediately
   - Do NOT write the plan with unresolved questions
   - The implementation plan must be complete and actionable
   - Every decision must be made before finalizing the plan

## Common Patterns

### For Database Changes:
- Start with schema/migration
- Add store methods
- Update business logic
- Expose via API
- Update clients

### For New Features:
- Research existing patterns first
- Start with data model
- Build backend logic
- Add API endpoints
- Implement UI last

### For Refactoring:
- Document current behavior
- Plan incremental changes
- Maintain backwards compatibility
- Include migration strategy

## Example Interaction Flow

```
User: /create_plan
Assistant: I'll help you create a detailed implementation plan...

User: We need to add parent-child tracking for agent sub-tasks. See docs/research-2025-01-08-parent-child-tracking.md
Assistant: Let me read that document completely first...

[Reads file fully]

Assistant: This is a repo-backed implementation plan, so I'll create three managed Surveil tasks, merge their reports, and use one <evidence-review-agent> to review the evidence before drafting.

[Interactive process continues...]
```
