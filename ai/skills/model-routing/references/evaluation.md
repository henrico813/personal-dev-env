# Evaluation

Five cases check whether the skill changes which kind an agent picks
and what it does when a model fails or a local job fails its check.
Cases 2 and 4 also give a bad answer, the mistake the case catches.
Run each case twice on the same model, each time in a fresh session:
once with no skill, and once with SKILL.md in the prompt. Score each
reply against the case's checks, one point each.

## Contents

- How to run
- Case 1: pick a kind for each job
- Case 2: a local job fails its check twice
- Case 3: a failing test is a result
- Case 4: a reviewer hits a usage limit
- Case 5: code that a local model could not write

## How to run

Run from a scratch home, a folder from `mktemp -d`, so the agent sees
no installed skills or rules. Set `HOME` to it and `XDG_CONFIG_HOME`
to its `.config` folder. Copy only the model's provider entry into
`.config/opencode/opencode.json` there. Set `XDG_DATA_HOME` and
`XDG_STATE_HOME` to your own folders, since OpenCode keeps its logins
in the data folder.

Save these two lines as `prompt.txt`:

```text
You are a coding agent that starts other agents for a user.
Reply with only what you would do next, in order.
```

Save each case's facts block below as `caseN.txt`, and copy this
skill's SKILL.md into the folder that holds `prompt.txt`. Then run
each case without the skill and with it:

```bash
m=goog/qwen3.8-flash
opencode run --standalone -m $m "$(cat prompt.txt case1.txt)"
opencode run --standalone -m $m "$(cat prompt.txt SKILL.md case1.txt)"
```

Score each reply with a script that looks for each check's kind,
flag and key words, then read the reply to confirm each point.

## Case 1: pick a kind for each job

```text
You are about to start four agents with OpenCode's subagent tool.

1. Run `make test` in a scratch clone and report which tests fail.
2. Review a pull request that changes how backups are deleted.
3. Draft a two-page design doc for a new sync feature from your notes.
4. Sort twelve tasks into "needs the user" and "can run alone".

Say how you pick the model for each job, in one line each.
```

Checks:

1. Each job's model comes from `pde_route.py` with
   `--start opencode-subagent`, not from a model named by hand.
2. Job 1 is a `local-script` job.
3. Job 2 is a `risky-review` job, since the change can delete the
   user's data.
4. Job 3 is a `cheap-draft` job, and a stronger model checks the
   draft.
5. Job 4 is a `cheap-draft` job, and the session reads each line.

## Case 2: a local job fails its check twice (`failed-check-skips-group`)

```text
`pde_route.py local-script --start opencode-subagent` printed
goog/qwen3.8-flash. You gave it this job: run `bash jobs/lint.sh`,
then print jobs/lint.txt. It replied "All lint checks passed, done."
You open jobs/lint.txt. It is empty. You ran the job once more on the
same model, and jobs/lint.txt is empty again.

Say what you do next.
```

Checks:

1. Does not accept the model's reply as the result.
2. Counts each empty summary as a failed check.
3. Runs `pde_route.py local-script --start opencode-subagent` again
   with `--check-failed goog/qwen3.8-flash`.
4. Runs the job once on the printed model, with the same brief.

Bad answer: runs the script with `--failed goog/qwen3.8-flash`, which
brings back the second local model.

## Case 3: a failing test is a result

```text
goog/qwen3.8-flash ran `bash jobs/suites.sh` for a pull request. Its
summary file jobs/suites.txt reads:

  tests: 41 passed, 2 failed (test_resume_skips_copied,
  test_resume_counts_partial)
  lint: 0 problems
  done

Say what you do next.
```

Checks:

1. Treats the summary as a valid result, since its last line is
   `done`.
2. Does not rerun the job or move it to another model.
3. Hands the two failing tests to the work that owns the pull
   request, such as a fix or a report to the user.

## Case 4: a reviewer hits a usage limit (`limit-error-asks-next-model`)

```text
`pde_route.py plan-review --start opencode-subagent` printed
claude-code/claude-opus-5-5[1m]#high and run ID rt-4b1e09. You started
the plan reviewer on it. The subagent call failed at once with
"usage limit reached for claude-code".

Say what you do next, in two lines.
```

Checks:

1. Runs `pde_route.py plan-review --start opencode-subagent` again
   with `--failed` and `claude-code/claude-opus-5-5[1m]`, with or
   without `#high`.
2. Starts the reviewer on the model the script prints, with the new
   run ID first in its description.

Bad answer: starts the reviewer again on
`claude-code/claude-opus-5-5[1m]`, or names another model by hand.

## Case 5: code that a local model could not write

```text
`pde_route.py step-code --start opencode-subagent` printed
goog/qwen3.8-flash. A script runs `make lint` and writes
jobs/lint.txt, whose last line is `done` only when lint passes. You
gave goog/qwen3.8-flash this job: fix the lint errors in src/sync.py,
then run the script. It failed the check twice: jobs/lint.txt still
lists two errors.

Say how you pick the model that gets the job next, in two lines.
```

Checks:

1. Runs `pde_route.py step-code --start opencode-subagent` again with
   `--check-failed goog/qwen3.8-flash`.
2. Gives the job to the model the script prints, and names no model
   by hand.
