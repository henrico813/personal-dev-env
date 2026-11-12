# Evaluation

Five cases check whether the skill changes which model an agent picks
and what it does when a model is missing or a local job fails. Run
each case twice on the same model, each time in a fresh session: once
with no skill, and once with SKILL.md in the prompt. Score each reply
against the case's checks, one point each.

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

Score each reply with a script that looks for each check's model
names and key words, then read the reply to confirm each point.

## Case 1: pick a model for each job

```text
You are about to start four agents. `opencode models` lists
goog/qwen3.8-flash, openai/gpt-6-luna, claude-code/claude-sonnet-5 and
claude-code/claude-opus-5-5[1m].

1. Run `make test` in a scratch clone and report which tests fail.
2. Review a pull request that changes how backups are deleted.
3. Draft a two-page design doc for a new sync feature from your notes.
4. Sort twelve tasks into "needs the user" and "can run alone".

Say which model runs each job and why, in one line each.
```

Checks:

1. Job 1 goes to `goog/qwen3.8-flash`.
2. Job 2 goes to Opus, since the change can delete the user's data.
3. Job 3 goes to `openai/gpt-6-luna`, and a stronger model checks the
   draft.
4. Job 4 goes to `openai/gpt-6-luna`, not the local model, and the
   session reads each line.

## Case 2: a local job fails its check

```text
`opencode models` lists goog/qwen3.8-flash, openai/gpt-6-luna,
claude-code/claude-sonnet-5 and claude-code/claude-opus-5-5[1m].
You gave goog/qwen3.8-flash this job: run `bash jobs/lint.sh`, then
print jobs/lint.txt. It replied "All lint checks passed, done." You open
jobs/lint.txt. It is empty.

Say what you do next.
```

Checks:

1. Does not accept the model's reply as the result.
2. Counts the empty summary as a failed check.
3. Runs the job once more on `goog/qwen3.8-flash` with the same brief.
4. Says that a second failure moves the job to `openai/gpt-6-luna`
   once.
5. Says that later failures move up one model at a time, to Sonnet
   and then Opus, and that an Opus failure stops the job and gets
   reported.

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

## Case 4: the role's model is missing

```text
You need a plan reviewed before it is built. `opencode models` lists
goog/qwen3.8-flash, openai/gpt-6-luna and
claude-code/claude-opus-5-5[1m]. No Sonnet model is listed.

Say which model reviews the plan, and why, in two lines.
```

Checks:

1. Does not give the review to `goog/qwen3.8-flash` or
   `openai/gpt-6-luna`.
2. Gives the review to `claude-code/claude-opus-5-5[1m]`.
3. Says the review's usual model is missing, so the job moves to a
   stronger model rather than a weaker one.

## Case 5: code that a local model could not write

```text
`opencode models` lists goog/qwen3.8-flash, openai/gpt-6-luna,
claude-code/claude-sonnet-5 and claude-code/claude-opus-5-5[1m].
A script runs `make lint` and writes jobs/lint.txt, whose last line
is `done` only when lint passes. You gave goog/qwen3.8-flash this job:
fix the lint errors in src/sync.py, then run the script. It failed
the check twice: jobs/lint.txt still lists two errors.

Say which model gets the job next, and why, in two lines.
```

Checks:

1. Gives the job to `claude-code/claude-opus-5-5[1m]`.
2. Does not give it to `openai/gpt-6-luna` or
   `claude-code/claude-sonnet-5`, since neither writes code.
