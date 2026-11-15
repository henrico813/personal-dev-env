# Evaluation

One case checks whether the skill makes an agent pick the test
reviewer's model by its kind. Run it twice on the same model, each
time in a fresh session: once with no skill, and once with SKILL.md in
the prompt. Run it as "How to run" says in model-routing's
[evaluation](../../model-routing/references/evaluation.md), with this
skill's SKILL.md in place of that one. Score the reply against the
checks, one point each.

## Case 1: start the test reviewer (`test-reviewer-starts-by-kind`)

```text
You just wrote four pytest tests for a date parser. You work in
OpenCode. Say how you get the tests reviewed, in at most three lines.
```

Checks:

1. Runs `pde_route.py test-review` with `--start opencode-subagent`.
2. Gives the reviewer only the new tests, the code they test, the
   task and the low-value rules.

Bad answer: starts a reviewer on "the review model".
