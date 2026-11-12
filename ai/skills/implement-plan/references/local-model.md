# Build on a local model

On a local-model build, the plan's own diffs become the commits, so
the local model `goog/qwen3.8` never writes code. It runs one script
per step, and the script commits the step's planned files and runs
the checks.
You run the plan checks, write the commit messages, handle steps that
fail and review the final diff.

## Contents

- Set up the build
- Run a step
- When a step fails
- Report

## Set up the build

1. Run the Freshness Check in SKILL.md and
   `planner check <plan.md> --repo <repo> --json-errors`. Stop on any
   error, as SKILL.md says.
2. Make the worktree as SKILL.md's non-Vibe execution says, with
   `--base <base commit>`, and make a run folder with `mktemp -d`.
   Below, `$D` stands for the run folder and `$W` for the worktree.
   Write their real paths into every file you write for the build.
3. Write the check command once, from the plan's Automated
   Verification: the tests and linters, run from `$W`, as one string
   such as `cd ai && uv run pytest -q`.
4. For each `### <step>.` heading in the plan, write the step's
   commit message to `$D/MSG<step>` by the `git-messages` skill, and
   run its `msg fmt`, then `msg lint`, on the file.

## Run a step

Run the steps in order, one at a time. For step `<n>`, write
`$D/step<n>.sh`:

```sh
cd "<worktree>" &&
  exec bash ~/.agents/skills/implement-plan/scripts/local-step.sh \
    "<plan.md>" <base commit> <n> "<run folder>/MSG<n>" \
    "<run folder>/job<n>" \
    '<check command>'
```

The script runs `planner check`, stops when the worktree has
changes, and gets the step's tree with `plan_tree` from roadmap-run's
`scripts/fns.sh`. It commits that tree on HEAD with the message, runs
the check command, and writes `$D/job<n>/summary.txt`. A rerun after
the commit skips it. The check output goes to `$D/job<n>/check.log`
and the planner output to `$D/job<n>/planner.log`.

Start the model from `$D` with the brief in roadmap-run's
[Local model](../../roadmap-run/references/prompts.md#local-model)
section, with `bash $D/step<n>.sh` as the script and
`$D/job<n>/summary.txt` as the summary file:

```sh
opencode run --standalone -m goog/qwen3.8 "<brief>"
```

The step passed only when all of these hold:

- The summary's last line is `done`.
- Its first line is `result: pass`.
- `git -C "$W" status --porcelain` prints nothing.
- `git -C "$W" log --oneline <HEAD before>..HEAD` shows at most the
  one step commit.

Never trust the model's own words about the result.

## When a step fails

1. On the first failure, run the same job on `goog/qwen3.8` once
   more. Undo nothing first, since the script resumes.
2. On the second failure, start a subagent on
   `claude-code/claude-opus-5-5[1m]#high`. Give it the step, the
   summary, `check.log`, `planner.log` and the worktree path. It finds
   the cause and never edits worktree files by hand.
3. When the code must change, the subagent corrects the plan's diff
   as SKILL.md's Plan Updates says. When the step's commit is already
   on the branch, it writes a fix message at `$D/MSGfix<n>` by
   `git-messages`. Then it reruns the step's `local-step.sh` command
   itself, with every argument the same except `$D/MSGfix<n>` in place
   of `$D/MSG<n>`. When the step has no commit yet, it reruns
   `bash $D/step<n>.sh`. The script commits the corrected tree, so
   the plan and branch stay the same. With a fix message, that step
   has a second commit on the branch, the fix.
4. When the cause is outside the code, such as a missing tool, the
   subagent fixes it and reruns `bash $D/step<n>.sh`.
5. The step counts as escalated. Check its summary as in Run a step
   before the next step starts.

After the last step, review the whole diff from the base commit as
SKILL.md's Completion says.

## Report

End the build with these lines:

```text
Passed locally: <count> (steps <numbers>)
Escalated: <numbers, each with why>
Opus tokens: <tokens read> read, <tokens written> written
qwen3.8 failure: <job>, goog/qwen3.8, <what went wrong>
```

Write one `qwen3.8 failure` line for each failed job. Take the
tokens from the `tokens read` and `tokens written` lines of
`python3 ~/.agents/skills/session-review/scripts/session_stats.py`,
run in this session. When a step was escalated, add the Opus line it
prints under subagents.
