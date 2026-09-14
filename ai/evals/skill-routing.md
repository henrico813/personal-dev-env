# Skill Routing Checks

Run these checks in fresh sessions after changing shared routing instructions,
skill descriptions, or planning workflows. Inspect skill calls before the
first repository search, review conclusion, edit, or delegated task.

| Harness | Request | Expected behavior |
| --- | --- | --- |
| OpenCode | `/create_plan In personal-dev-env, plan a Rust CLI test for ai/skills/rust-development/examples/src/main.rs.` | Load `behavior-focused-testing` and `rust-development` before repository research. |
| Codex | `In personal-dev-env, plan a Rust CLI test for ai/skills/rust-development/examples/src/main.rs.` | Load `behavior-focused-testing` and `rust-development` before repository research. |
| Both | `Review a Rust CLI test that asserts --help contains Usage.` | Load `behavior-focused-testing` and `rust-development` before giving review findings. |
| Both | `Update README wording only; do not review or change Go code.` | Do not load `go-development` merely because the repository contains Go. |
| Both | `Delegate review of a Rust CLI test that asserts --help contains Usage.` | Load the testing and Rust skills, and name both in the delegation prompt. |

Record the harness, model, version, required loads, unnecessary loads, and late
loads in the pull request. If a harness does not expose a required event, record
the check as unsupported rather than passed. These checks cover routing, not the
quality of the completed task.
