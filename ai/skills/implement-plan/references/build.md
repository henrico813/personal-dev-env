# Build a plan with the step script

A build spends as few tokens as it can. The plan's own diffs become
the commits, so no model writes code. A script commits each step's
planned files and runs the checks. Each step is a local job on the
local model, as `model-routing` picks. A stronger model steps in only
to fix a step that the local model could not pass.
You run the plan checks, write the commit messages, handle steps that
fail and review the final diff.

## Contents

- Who runs the steps
- Set up the build
- Run a step
- When a step fails
- Report

## Who runs the steps

1. Each step's script runs as a local job, on the local model, as
   `model-routing` picks. That skill also says how to check that the
   model answers, and which model runs the job when it does not.
2. When that model is the model running this session, run each
   step's script yourself instead of starting a job for it.

## Set up the build

1. Run the Freshness Check in SKILL.md and
   `planner check <plan.md> --repo <repo> --json-errors`. Stop on any
   error, as SKILL.md says.
2. Make the worktree as SKILL.md's Freshness Check says, with
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
  exec bash ~/.agents/skills/implement-plan/scripts/build-step.sh \
    "<plan.md>" <base commit> <n> "<run folder>/MSG<n>" \
    "<run folder>/job<n>" \
    '<check command>'
```

The script runs `planner check`, stops when the worktree has
changes, and gets the step's tree with `plan_tree` from auto-run's
`scripts/fns.sh`. It commits that tree on HEAD with the message, runs
the check command, and writes `$D/job<n>/summary.txt`. A rerun after
the commit skips it. The check output goes to `$D/job<n>/check.log`
and the planner output to `$D/job<n>/planner.log`.

Before the job starts, save the commit that HEAD points to. The pass
check below calls it `<HEAD before>`:

```sh
git -C "$W" rev-parse HEAD
```

Start the local model from `$D` with the brief that `model-routing`
gives for a local job. In that brief, use `bash $D/step<n>.sh` as the
script and `$D/job<n>/summary.txt` as the summary file:

```sh
opencode run --standalone -m <local model> "<brief>"
```

When the local model is this session's model, as Who runs the steps
says, run `bash $D/step<n>.sh` yourself instead, and check it the same
way.

The step passed only when all of these hold:

- The summary's last line is `done`.
- Its first line is `result: pass`.
- `git -C "$W" status --porcelain` prints nothing.
- `git -C "$W" log --oneline <HEAD before>..HEAD` shows at most the
  one step commit.

Never trust the model's own words about the result.

## When a step fails

A step fails in one of two ways:

- The check failed: the summary is missing, its last line is not
  `done`, the worktree has changes, or the log shows more than one
  new commit. Run the step once more on the same model, as
  `model-routing` says for a failed check. Undo nothing before the
  retry, since the script resumes. When the retry fails too, fix the
  step as below.
- The summary ends in `done` but starts with `result: fail`, such as
  when tests fail. That is a failed result, not a failed check. Fix
  the step as below without a retry.

A fix is its own job. This skill sets the fix's role. A fix can
change the plan, so the local model never makes one. The review model
writes no code, as `model-routing` says, so the fix runs on the
strongest model.

1. Start a subagent on the strongest model, as `model-routing` picks.
   Give it the step, the summary, `check.log`, `planner.log`, the
   worktree path, and an `Applicable skills:` line with each loaded
   skill the step needs. It finds the cause and never edits worktree
   files by hand.
2. When the code must change, the subagent corrects the plan's diff
   as SKILL.md's Plan Updates says. When the step's commit is already
   on the branch, it writes a fix message at `$D/MSGfix<n>` by
   `git-messages`. Then it reruns the step's `build-step.sh` command
   itself, with every argument the same except `$D/MSGfix<n>` in place
   of `$D/MSG<n>`. When the step has no commit yet, it reruns
   `bash $D/step<n>.sh`. The script commits the corrected tree, so
   the plan and branch stay the same. With a fix message, that step
   has a second commit on the branch, the fix.
3. When the cause is outside the code, such as a missing tool, the
   subagent fixes it and reruns `bash $D/step<n>.sh`.
4. When the subagent cannot make the step pass, stop and report the
   step as blocked.
5. The step counts as escalated. Check its summary as in Run a step
   before the next step starts.

After the last step, review the whole diff from the base commit as
SKILL.md's Completion says.

## Report

End the build with these lines:

```text
Passed on <model>: <count> (steps <numbers>)
Escalated: <step>, <model it moved to>, <why>
Tokens: <model>, <tokens read> read, <tokens written> written
<model> failure: <job>, <what went wrong>
```

- Write one `Escalated` line each time a step moves to a stronger
  model.
- Write one `Tokens` line for each model that ran. Take the tokens of
  this session and of each subagent model from
  `python3 ~/.agents/skills/session-review/scripts/session_stats.py`,
  run in this session.
- A step job started with `opencode run` is its own session, so the
  script does not count it. For the local model, write
  `Tokens: <local model>, <count> jobs` instead, unless this session
  ran the script itself.
- Write one failure line for each failed job or subagent.
