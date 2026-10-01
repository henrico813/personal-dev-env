# Evaluate this skill

Use these cases when installing or revising the skill, not during every Python
change. Run them in the intended agent and inspect activation, reference loads,
the actual diff, and executed checks. These are proposed cases, not evidence of
completed agent testing.

## Everyday behavior

| Request | Expected behavior |
| --- | --- |
| Add configuration parsing to this Python CLI. | Reads existing APIs and tests, keeps parsing testable, and does not invent a provider framework. |
| Fix this Python type-checker error. | Establishes runtime behavior first, narrows or corrects the type, and does not cast blindly. |
| Review this Python module for maintainability. | Checks behavior, names, errors, resources, tests, and supported versions; separates defects from preferences. |
| Add tests for this Python function. | Loads shared test guidance, uses pytest when the repository runs it, keeps inputs visible, and parameterizes only comparable cases. |
| Make this external operation testable. | Uses real deterministic parts and substitutes the external dependency only where needed. |
| Organize this growing Python CLI. | Groups by responsibility, keeps process adaptation small, and avoids one module per class. |

## Reference-loading checks

| Request | Must read | Expected behavior |
| --- | --- | --- |
| Plan a missing-status fix for an API wrapper. | Design choices, API design | Proposes focused validation and regression cases; preserves the client interface and does not add a registry or wrapper. |
| Review a ten-million-job async runner that creates one task per ID. | Concurrency, design choices | Distinguishes task, queue, and active-I/O limits; proposes incremental input, fixed ownership, and checked state transitions. |
| Design parse-worker tests for values 1 through 64. | Testing | Uses pytest, named parameters, literal expectations, and separate assertion paths without a flag-driven mini-framework. |
| Review a fixture whose third setup call can fail. | Testing, errors and resources | Protects earlier acquisitions, accounts for partial remote effects, and does not claim split fixtures make setup atomic. |
| Refactor a Python 3.10 library whose CI runs `python -m unittest` and has no pytest dependency. | Testing, typing, packaging | Uses supported syntax. Keeps new and touched tests in unittest style, does not add pytest or change CI, runs the configured unittest command, and notes the limitation. |
| Review broad exception catching and a vague docstring. | Errors and resources, documentation | Preserves useful failures, applies proportional docs, and makes retry inputs explicit without adding a container framework. |
| Explain why patching `os.listdir` misses an imported name. | Testing | Explains name binding and patches the module lookup site; does not change production imports solely for the mock. |
| A reusable object-store fake has drifted twice. | Testing, design choices | Keeps fast tests, adds a small shared behavior suite with focused real coverage, and does not claim complete equivalence. |

## Negative and constraint cases

| Request or condition | Expected behavior |
| --- | --- |
| Explain what a Python list is. | Does not activate automatically for a basic syntax question. |
| Only run the configured formatter on this Python file. | Does not turn the request into design review or broader cleanup. |
| Fix spelling in this README; do not change code. | Does not activate merely because the repository contains Python. |
| Review this Go HTTP server. | Does not apply this Python skill. |
| The repository supports Python 3.10. | Avoids TaskGroup and newer annotation syntax unless compatibility code or a version change is requested. |
| No interpreter or external service is available. | Reports unrun checks and separates inspection from executed evidence. |

For each positive case, inspect whether required references were read before the
decision, whether shared testing and documentation skills loaded when applicable,
and whether unrelated changes appeared. Run each case with and without the skill
on every applicable target: OpenCode with `opencode-go/qwen3.6-plus` and
`opencode-go/gpt-5.6-luna`, Codex with `gpt-5.6-luna`, and Pi with
`openai-codex/gpt-5.6-luna`. Record the harness, exact model and version,
activation, reference loads, output quality, unsupported providers, and
differences from the baseline.

## Validate the example package

| Check | Expected result |
| --- | --- |
| Parse `SKILL.md` frontmatter. | Name and multiline description load without extra metadata. |
| Run the local link checker. | Every relative file path and heading fragment resolves. |
| Run Ruff formatting and lint checks. | All example source and tests pass configured checks. |
| Run mypy. | Strict checking passes for `src` and `tests`. |
| Run pytest. | Unit, process, async, and configured doctest cases pass. |
| Run `uv build`. | Both source and wheel distributions build from the locked project. |
| Run checks on Python 3.12 and 3.13. | The declared minimum and the managed newer interpreter both pass. |

Follow [the example README](../examples/README.md) for exact commands. Review the
success and failure tests, not only the exit status. Static inspection, link
validation, and package builds do not establish agent activation or decision
quality.
