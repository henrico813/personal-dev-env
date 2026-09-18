# Skill Routing Checks

Run these checks in fresh sessions after changing shared routing instructions,
skill descriptions, or planning workflows. After directly referenced context is
read, inspect skill calls before broader repository research, a review
conclusion, an edit, or a delegated task.

Run OpenCode rows with both `opencode-go/qwen3.6-plus` and
`opencode-go/gpt-5.6-luna`, passing the selector with `--model`. Run Codex rows with
`gpt-5.6-luna`; the corresponding Pi selector is
`openai-codex/gpt-5.6-luna`. Record an unavailable provider as unsupported
rather than silently substituting a stronger model.

Before the OpenCode planning check, allocate a unique output directory with
`mktemp -d "${TMPDIR:-/tmp}/skill-routing.XXXXXX"` and replace `<output-dir>`
below with the printed path. First install and compare the planning workflows as
described in `plan-workflows.md`; run the check in a fresh session.

| Harness | Request | Expected behavior |
| --- | --- | --- |
| OpenCode | `/create_plan In personal-dev-env, plan a Rust CLI test for ai/skills/rust-development/examples/src/main.rs and write it to <output-dir>/rust-plan.md.` | After reading the referenced file, load `behavior-focused-testing`, `code-documentation`, and `rust-development` before broader repository research. Name all three in any planning delegation; do not create a reviewer solely for skill routing. |
| Codex | `In personal-dev-env, plan a Rust CLI test for ai/skills/rust-development/examples/src/main.rs.` | After reading the referenced file, load `behavior-focused-testing`, `code-documentation`, and `rust-development` before broader repository research. |
| Both | `Review a Rust CLI test that asserts --help contains Usage.` | Load `behavior-focused-testing`, `code-documentation`, and `rust-development` before giving review findings. |
| Both | `Update README wording only; do not review or change Go code.` | Do not load `go-development` or `code-documentation` merely because the repository contains source code. This also applies through the documentation workflow. |
| Both | `Delegate review of a Rust CLI test that asserts --help contains Usage.` | Load the testing, code-documentation, and Rust skills, and name all three in the delegation prompt. |
| Both | `Review this function and its docstring for maintainability.` | Load `code-documentation` before giving findings. |
| Both | `Create a Zettel that captures the Rust CLI test review findings.` | Load `obsidian-zettel` before creating a vault note. |

Record the harness, model, version, required loads, unnecessary loads, late
loads, and delegated loads in the pull request. If a harness does not expose a
required event, record the check as unsupported rather than passed. These
checks cover routing, not the quality of the completed task; use
`plan-workflows.md` for completed-plan behavior.

Record Surveil's exact managed run path. After trace inspection, delete that
captured run and the allocated output directory; do not search with globs.
