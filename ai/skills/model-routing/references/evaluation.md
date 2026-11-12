# Evaluation

Three cases check whether the skill changes which model an agent
picks and what it does when a local job fails. Run each case twice in
a fresh session on the same model: once with no skill, and once with
SKILL.md pasted after the system prompt. Score each reply against the
case's checks, one point each.

## How to run

Run in a folder from `mktemp -d`. `--safe-mode` leaves out the user's
CLAUDE.md and installed skills, so the baseline sees no rules.

```bash
sys='You are a coding agent that starts other agents for a user.
Reply with only what you would do next, in order.'
claude -p --safe-mode --model claude-sonnet-5 --tools "" \
  --no-session-persistence --system-prompt "$sys" "$(cat case1.txt)"
```

For the run with the skill, copy this skill's SKILL.md into the
folder. This command puts it at the end of the system prompt:

```bash
claude -p --safe-mode --model claude-sonnet-5 --tools "" \
  --no-session-persistence --system-prompt "$sys

$(cat SKILL.md)" "$(cat case1.txt)"
```

Save each case's facts block below as `caseN.txt`.

## Case 1: pick a model for each job

```text
You are about to start four agents. `opencode models` lists
goog/qwen3.8, openai/gpt-6-luna and claude-code/claude-opus-5-5[1m].

1. Run `make test` in a scratch clone and report which tests fail.
2. Review a pull request that changes how backups are deleted.
3. Draft a two-page design doc for a new sync feature from your notes.
4. Sort twelve tasks into "needs the user" and "can run alone".

Say which model runs each job and why, in one line each.
```

Checks:

1. Job 1 goes to `goog/qwen3.8`.
2. Job 2 goes to Opus.
3. Job 3 goes to `openai/gpt-6-luna`, with an Opus check after.
4. Job 4 stays on a cloud model, because no script can check it.

## Case 2: a local job fails its check

```text
You gave goog/qwen3.8 this job: run `bash jobs/lint.sh`, then print
jobs/lint.txt. It replied "All lint checks passed, done." You open
jobs/lint.txt. It is empty.

Say what you do next.
```

Checks:

1. Does not accept the model's reply as the result.
2. Counts the empty summary as a failed check.
3. Runs the job once more on `goog/qwen3.8` with the same brief.
4. Says that a second failure moves the job to `openai/gpt-6-luna`
   once.
5. Says that a third failure moves the job to Opus once, and an Opus
   failure stops the job and gets reported.

## Case 3: a failing test is a result

```text
goog/qwen3.8 ran `bash jobs/suites.sh` for a pull request. Its
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
