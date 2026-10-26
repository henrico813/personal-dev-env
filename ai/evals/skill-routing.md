# Skill Routing Checks

Run these checks in fresh sessions after changing shared routing instructions,
skill descriptions, or planning workflows. After directly referenced context is
read, inspect skill calls before broader repository research, a review
conclusion, an edit, or a delegated task.

Set the model once for every row: `EVAL_MODEL="${EVAL_MODEL:-goog/qwen3.8}"`.
Pass it with `--model` to OpenCode and Pi/Vibe. Record an unavailable model as
unsupported rather than silently substituting a stronger model.

Before the planning check, allocate a unique output directory with
`mktemp -d "${TMPDIR:-/tmp}/skill-routing.XXXXXX"` and replace `<output-dir>`
below with the printed path. First install and compare the planning workflows as
described in `plan-workflows.md`; run the check in a fresh session.

| Harness | Request | Expected behavior |
| --- | --- | --- |
| OpenCode | `/create_plan In personal-dev-env, plan a Rust CLI test for ai/skills/rust-development/examples/src/main.rs and write it to <output-dir>/rust-plan.md.` | After reading the referenced file, load `behavior-focused-testing`, `code-documentation`, and `rust-development` before broader repository research. Name all three in any planning delegation; do not create a reviewer solely for skill routing. |
| OpenCode | `Review ai/skills/rust-development/examples/tests/cli.rs::missing_count_shows_usage without editing files.` | Load `behavior-focused-testing`, `code-documentation`, and `rust-development` before giving review findings. |
| OpenCode | `/create_plan In personal-dev-env, plan a Python test for ai/skills/python-development/examples/src/python_skill_examples/async_jobs.py and write it to <output-dir>/python-plan.md.` | After reading the referenced file, load `behavior-focused-testing`, `code-documentation`, and `python-development` before broader repository research. Name all three in any planning delegation. |
| OpenCode | `Review ai/skills/python-development/examples/tests/test_async_jobs.py::test_work_and_tasks_stay_bounded without editing files.` | Load `behavior-focused-testing`, `code-documentation`, and `python-development` before giving review findings. |
| OpenCode | `Explain what a Python list is.` | Do not load `python-development` for a basic syntax question with no planning, implementation, debugging, review, or test work. |
| OpenCode | `Update README wording only; do not review or change Go code.` | Do not load `go-development` or `code-documentation` merely because the repository contains source code. This also applies through the documentation workflow. |
| OpenCode | `Delegate a read-only review of ai/skills/rust-development/examples/tests/cli.rs::missing_count_shows_usage.` | Load the testing, code-documentation, and Rust skills, and name all three in the delegation prompt. |
| OpenCode | `Review vibe/src/adapters/docker.rs::prepare_provider_auth and its nearby source documentation without editing files.` | Load `code-documentation` and `rust-development` before giving findings. |
| OpenCode | `Review PR 253 without editing files.` | Load `review-plan` before giving review findings. |
| OpenCode | `Create a Zettel that captures the Rust CLI test review findings.` | Load `obsidian-zettel` before creating a vault note. |
| OpenCode, delegated | `Write an implementation plan for one new CLI test for ai/skills/rust-development/examples/src/main.rs that covers behavior tests/cli.rs does not cover yet. Write it to <output-dir>/rust-plan.md.` | Load `create-plan`, `behavior-focused-testing`, `code-documentation`, and `rust-development` before writing the plan. |
| OpenCode, delegated | `Review PR 253 without editing files.` | Load `review-plan` at some point in the session. |
| OpenCode | `Build the remaining PRs of the roadmap in <output-dir>/roadmap.md overnight; I will review them in the morning. Do not start agents, push, or file PR requests.` | Load `roadmap-run` before planning. The roadmap does not exist, so ask for an approved design and roadmap and stop. |
| OpenCode | `Have an agent write a plan for one new CLI test for ai/skills/rust-development/examples/src/main.rs and review it until it is ready for me. Do not start agents.` | Load `delegate`, not `roadmap-run`: one task is not a roadmap run. |
| OpenCode | `Why is this comment hard to read? pde-comment-review/internal/review/proposals.go, the doc comment on ParseProposals. Do not edit files.` | Load `plain-direct-english` before answering, break the comment down part by part, and edit nothing. |
| OpenCode | `Fix the typo in this sentence and change nothing else: "The parser will recieve the file."` | Reply with "receive" fixed and nothing else changed, without loading `plain-direct-english`. |
| OpenCode | `Add a summary of this session to today's daily note.` | Load `obsidian-daily-note` and show the exact block for approval before writing. Decline at approval so nothing is written. |
| OpenCode | `What did I write in today's daily note?` | Does not load `obsidian-daily-note` and does not read the note (vault AGENTS.md forbids `300 Journal/`). |

Record the harness, model, version, required loads, unnecessary loads, late
loads, and delegated loads in the pull request. If a harness does not expose a
required event, record the check as unsupported rather than passed. These
checks cover routing, not the quality of the completed task; use
`plan-workflows.md` for completed-plan behavior.

## Delegated Skill Loading

The delegated rows check that an agent given only its task loads the right
skills. Subagents use the real installation, not the temporary home in
`plan-workflows.md`, so install from the pull request's worktree and
reinstall from main afterwards:

```bash
installer=$(mktemp /tmp/pde-installer.XXXXXX)
go build -C pde-installer -o "$installer" .
PDE_REPO_ROOT="$PWD" "$installer" install full
```

Start the top-level session in that worktree, so the plan row's paths and
`gh` resolve there. Run each row once per model you delegate to, such as
`openai/gpt-5.6-luna` or `claude-code/claude-sonnet-5`. Run `goog/qwen3.8`
rows in their own batch, since they share the local LiteLLM server, and
record a model that cannot be reached as unsupported.

Subagents cannot delegate, so send each prompt from a top-level session with
the subagent tool, set its `model` field, and add no `Applicable skills:` line.
Then audit the subagent's session, whose ID is the `task_id` the subagent tool
returns:

```bash
python3 ai/evals/skill_audit.py <session-id>
```

It prints the skills read before and after the first edit and whether
`git push` ran. A plan run passes when every expected skill is in `before=` and
`push=no`. A review run passes when each expected skill is in `before=` or
`after=` and `push=no`, since reviewers often save repository status to a
scratch file first. The review row checks only workflow routing, so a review
that skips the Go or testing skills still passes. If a skill is missed, rerun
once with an `Applicable skills:` line naming the expected skills and record
both runs. Claude Code models also load the owner's global instructions and
memory, so their runs see more than the task.
