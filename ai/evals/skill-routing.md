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
| OpenCode | `Create a Zettel that captures the Rust CLI test review findings.` | Load `obsidian-zettel` before creating a vault note. |

Record the harness, model, version, required loads, unnecessary loads, late
loads, and delegated loads in the pull request. If a harness does not expose a
required event, record the check as unsupported rather than passed. These
checks cover routing, not the quality of the completed task; use
`plan-workflows.md` for completed-plan behavior.

