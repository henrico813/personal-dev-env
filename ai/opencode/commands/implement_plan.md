---
description: Implement an approved design document with verification
---

# Implement Plan

You are tasked with implementing an approved design document. These plans contain phases with specific changes and success criteria.

## Getting Started

When given a plan reference:
- Read the plan completely and check for any existing checkmarks (- [x])
- Read the original ticket and all files mentioned in the plan
- **Read files fully** - never use limit/offset parameters, you need complete context
- Resolve the plan reference using the Planning Docs guidance in `ai/AGENTS.md`.
- Use existing filesystem paths directly.
- Otherwise resolve the plan file with `pde vault locate --json --vault <selector> "<reference>"`.
- If the request only identifies a vault root, ask for the actual plan reference instead of treating the directory as a plan.
- Think deeply about how the pieces fit together
- Create a todo list to track your progress
- By default, create a new branch and new worktree for your work, unless the plan specifies otherwise.
- The default branch name should follow conventional branch structure, e.g., `feature/short-description` or `bugfix/xxxx-###/short-description`, where xxxx is the plan id and ### is the plan number. If the plan specifies a branch name, use that instead.
- The default worktree path should be `./worktrees/name`, where name is derived from the plan title first, then the branch name otherwise. If the plan specifies a worktree path, use that instead.
- Start implementing if you understand what needs to be done

If no plan reference is provided, ask for one.

## Detailed Implementation Strategy

When `vibe` is available, prefer Vibe mode for implementation plans. Use the
`vibe` CLI directly as the execution surface.

Before using Vibe, inspect the installed CLI:

```bash
which vibe
vibe --help
vibe run --help
```

Before using `vibe run`, ensure provider auth is configured through one of
the supported env vars or `~/.pi/agent/auth.json`. Missing auth should fail
as `setup_error`, not a generic agent failure.

For this workflow, <task-context> is the complete plan and every document it references. For repo-backed plans, run the following research once per plan, not once per implementation step.

For this workflow, <evidence-review-agent> is the OpenCode `codebase-analyzer` agent.

## Detailed Surveil Research Instructions

1. Create the three Surveil tasks:
   - Before any Surveil command, run `failure_file="$(mktemp "${TMPDIR:-/tmp}/surveil-research-failure.XXXXXX")"` to reserve a unique fallback failure file.
   - Run `search_dir="$(surveil new task --task architecture)"`.
   - After root creation succeeds, run `rm -f "$failure_file"` and then `failure_file="$search_dir/failure.md"`.
   - Run `surveil new task --root "$search_dir" --task interfaces-data-state`.
   - Run `surveil new task --root "$search_dir" --task tests-verification`.
2. Populate `$search_dir/architecture/task.json`, `$search_dir/interfaces-data-state/task.json`, and `$search_dir/tests-verification/task.json` from <task-context>:
   - Set `summary` to the task-context title; if it has no title, use its first sentence verbatim.
   - Set `explicit_files` to only literal paths named by the task context, preserving first-seen order and removing exact duplicates.
   - Set `search_areas` to the smallest repo directories covering those paths and each task's focus; use `.` only when the intended scope is the repository root.
   - Treat every relative `explicit_files` and `search_areas` value as relative to the exact <repo> passed to `--repo`; recalculate them if <repo> changes.
   - Set `terms` to literal identifiers, filenames, path segments, commands, and feature names, de-duplicate case-insensitively, and do not invent synonyms.
3. Populate each task's `query` array with its ordered questions. Do not omit, reorder, combine, reword, or reuse question sets across tasks.
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
4. Run `surveil index --repo <repo>`.
5. Run all three gather commands:
   - `surveil gather --repo <repo> --task-file "$search_dir/architecture/task.json" > "$search_dir/architecture/context.json"`
   - `surveil gather --repo <repo> --task-file "$search_dir/interfaces-data-state/task.json" > "$search_dir/interfaces-data-state/context.json"`
   - `surveil gather --repo <repo> --task-file "$search_dir/tests-verification/task.json" > "$search_dir/tests-verification/context.json"`
6. Launch all three research commands through parallel tool calls and wait for all three:
   - `surveil research --context "$search_dir/architecture/context.json" --trace-out "$search_dir/architecture/trace.json" > "$search_dir/architecture/report.json"`
   - `surveil research --context "$search_dir/interfaces-data-state/context.json" --trace-out "$search_dir/interfaces-data-state/trace.json" > "$search_dir/interfaces-data-state/report.json"`
   - `surveil research --context "$search_dir/tests-verification/context.json" --trace-out "$search_dir/tests-verification/trace.json" > "$search_dir/tests-verification/report.json"`
7. Proceed to step 8 only after all three tasks succeed; otherwise follow step 12.
8. Merge the reports directly with `surveil merge "$search_dir/architecture/report.json" "$search_dir/interfaces-data-state/report.json" "$search_dir/tests-verification/report.json" > "$search_dir/evidence.json"`.
9. Read `$search_dir/evidence.json` before additional repository research.
10. After successful evidence, run one <evidence-review-agent>:
    - Give it <task-context>, <repo>, and `$search_dir/evidence.json`.
    - Find required files or behavior missing from the evidence and correct assumptions not supported by direct file reads.
    - Check related callers, integration points, and existing patterns outside the searched areas.
    - Identify missing tests, fixtures, config, commands, CI checks, or manual verification.
    - Require read-only research with concrete `file:line` references and findings not already present in the evidence.
    - Save its final response verbatim as `$search_dir/manual-review.md`.
11. Verify new or conflicting findings from <evidence-review-agent> with direct file reads before continuing.
12. If task JSON or search-area validation fails, correct the input and rerun gather; input corrections do not consume the operational retry. If any other Surveil command fails, retry it once. If it still fails, write the failed stage to `$failure_file`, skip steps 8-9 only, run one <evidence-review-agent> using all step 10 review instructions with <task-context>, <repo>, and any available artifacts, save its response beside `$failure_file`, and verify new or conflicting fallback findings with direct file reads before continuing.

Read the plan fully, then run one unchecked implementation step at a time.
Create a unique prompt directory for that step with `prompt_dir="$(mktemp -d "${TMPDIR:-/tmp}/implement-plan-prompt.XXXXXX")"`.
Write the exact current plan step verbatim as raw Markdown to `"$prompt_dir/prompt.md"`, then invoke the `vibe` CLI
directly with the current run flow:

```bash
vibe run --key "$KEY" --prompt-file "$prompt_dir/prompt.md" --model "$MODEL"
```

The skill owns step selection and prompt preparation. Vibe owns worktree
creation, sandboxing, execution, and result reporting.

Do not create the branch or worktree manually in Vibe mode; Vibe owns that
setup.

After each run, parse the final JSON and review the result before continuing.

Handle statuses as follows:
- `completed`: inspect the commit, run verification, update the plan, then continue.
- `noop`: continue only if the step was already complete or intentionally no-op.
- `agent_failed`, `commit_failed`, `refused_dirty`, `setup_error`: stop, inspect the reported logs/worktree, and notify the user.

For completed steps, review correctness and consistency before running the next
step:
- changes match the plan step
- no unrelated files changed
- no secrets or generated artifacts were committed
- verification passes

Use the planner CLI first for plan updates. Refresh `update_diff_expect` via `planner inspect` before any `Update Diff` edit. Update goals and verification checkboxes as work completes, but do not add checkboxes to implementation steps. Direct checkbox edits are allowed only for the allowed checkbox cases; all other plan changes must go through the planner CLI. After plan updates, run `planner check <plan.md> --json-errors`. If the plan cannot be parsed, stop and escalate to the user. Do not mark manual verification complete unless the user confirms it.

When finished, summarize:
- steps run
- statuses
- commits
- worktree/branch
- verification results
- log/artifact paths
- any remaining manual checks or risks

## Implementation Philosophy

Plans are carefully designed, but reality can be messy. Your job is to:
- Follow the plan's intent while adapting to what you find
- Implement each phase fully before moving to the next
- Verify your work makes sense in the broader codebase context
- Update checkboxes in the plan as you complete sections

When things don't match the plan exactly, think about why and communicate clearly. The plan is your guide, but your judgment matters too.

If you encounter a mismatch:
- STOP and think deeply about why the plan can't be followed
- Present the issue clearly:
  ```
  Issue in Phase [N]:
  Expected: [what the plan says]
  Found: [actual situation]
  Why this matters: [explanation]

  How should I proceed?
  ```

Always write code documentation for tests, classes, functions, modules, unclear code sections, or system critical paths. If it's obvious what the things does, ask yourself is it clear why the thing exists and how it fits into the bigger picture.

If your code change affects something that is already documented in the code, prefer updated the code documentation already present based on the plan. Then update the plan to reflect the new documentation.

Humans understand writing best when it's presented as a story, narrative, or history. Keep prose optimized for human understanding. Humans understand writing best when it's presented as a story, narrative, or history. Prose should flow like a narrative, not a taxonomy. It should tell the story of why the changes are needed, how they fit together, and what the expected outcome is. Keep code documentation concise and focused on the "why" behind the code, not just the "what". The code blocks should include comments that explain the intent of the code in relation to the overall plan.

Software engineering documentation (SEDs) is defined as the following documentation types:

1. Docstrings and comments in code files for modules, classes, functions, unclear code sections, or system critical paths.
2. Implementation plans
3. Git documentation
4. Research Docs
5. Design Docs
6. Code Documentation

Here are the general questions you should ask yourself for guidance when writing SEDs:

- Why is this important?
- What is this for?
- What is the historical context behind this?
- How does this work?
- How does this fit into the system as a whole and how does it relate to other systems?
- Does the text flow like a narrative that expresses the intent of the requirements or design?


## Verification Approach

After implementing a phase:
- Run the success criteria checks (usually `make check test` covers everything)
- Fix any issues before proceeding
- Update your progress in both the plan and your todos
- Update goals and verification checkboxes as work completes; do not add checkboxes to implementation steps
- Direct checkbox edits are allowed only for the allowed checkbox cases; all other plan changes must go through the planner CLI
- After plan updates, run `planner check <plan.md> --json-errors`
- **Human verification**: Do not pause by default. If a step matches the plan and verification passes, continue. Perform manual verification yourself when feasible.

Pause only if something went wrong, a required manual check is not feasible for the agent, or the user explicitly asked for a stop or custom condition. User-provided execution instructions override this default.

do not check off items in the manual testing steps until confirmed by the user.


## If You Get Stuck

When something isn't working as expected:
- First, make sure you've read and understood all the relevant code
- Consider if the codebase has evolved since the plan was written
- Present the mismatch clearly and ask for guidance

Use sub-tasks sparingly - mainly for targeted debugging or exploring unfamiliar territory.

## Resuming Work

If the plan has existing checkmarks:
- Trust that completed work is done
- Pick up from the first unchecked item
- Verify previous work only if something seems off

Remember: You're implementing a solution, not just checking boxes. Keep the end goal in mind and maintain forward momentum.

## Cleaning Up

After completing all phases, present your work to the user, present a commit message, and present a PR message. The commit message should always follow conventional commit structure and by default contain a high level summary of the changes explaining what changed, why it changed, and how it's important written in present tense.

The PR message must contain an # Overview and # Testing section. The overview should mirror the commit message body or have a high level summary of all commit messages in a change.

The testing section should contain a detailed description of how to test the change, including any manual testing steps that need to be performed.

The primary audience for your commit and PR messages is a reviewer or a maintainer. They don't need deep implementation details. They need to understand the high level changes, why they were made, and how to verify them.
