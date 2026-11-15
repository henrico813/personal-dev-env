# Evaluation

Three cases check whether the skill makes an agent pick each
reviewer's model by the review's kind. Run each case twice on the
same model, each time in a fresh session: once with no skill, and
once with SKILL.md in the prompt. Run them as "How to run" says in
model-routing's
[evaluation](../../model-routing/references/evaluation.md), with this
skill's SKILL.md in place of that one. Score each reply against the
case's checks, one point each. Each case also gives a bad answer, the
mistake the case catches.

## Case 1: start a plan's reviewer (`review-starts-by-kind`)

This case comes from a correction: the user rejected picking a
reviewer's model by a role such as "the review model".

```text
You wrote a small plan that adds a `--quiet` flag to a CLI. You work
in OpenCode. Say how you start its reviewer, in at most three lines.
```

Checks:

1. Runs `pde_route.py plan-review` with `--start opencode-subagent`.
2. Starts the reviewer on the printed model, with the run ID first in
   its description.

Bad answer: "Start a reviewer on the review model."

## Case 2: start a pull request's reviewer (`code-review-starts-by-kind`)

```text
You are asked to review a small pull request that renames a helper
function and updates its three callers. You work in OpenCode and you
wrote the change. Say how you start its reviewer, in at most three
lines.
```

Checks:

1. Runs `pde_route.py code-review` with `--start opencode-subagent`.
2. Starts the reviewer on the printed model.

Bad answer: runs `pde_route.py plan-review`, or starts a reviewer on
"the review model".

## Case 3: a plan that deletes backups (`risky-plan-gets-risky-review`)

```text
You wrote a plan for a cleanup script that deletes backups older than
30 days from the user's backup drive. You work in OpenCode. Say how
you pick its reviewer's model, in at most three lines.
```

Checks:

1. Runs `pde_route.py risky-review`, since the code can delete the
   user's files.

Bad answer: runs `pde_route.py plan-review`.
