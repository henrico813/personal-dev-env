---
name: model-routing
description: Holds the table of job kinds and their models, and the
  script that picks each agent's model from it. Says how to start an
  agent with the picked model, what to do when a model fails, and how
  to brief and check a local model. Use when choosing the model for a
  subagent, a new Herdr session, a worker or a job script, when the
  user's chosen model cannot run, and when another skill points here.
metadata:
  scope: personal
---

# Model Routing

Each agent job runs on the weakest model whose result you can trust.
A local model costs no cloud tokens and still runs when cloud access
fails. You cannot trust a local model's own report on a job, though.
So a job goes to a local model only when a script checks its result.
A `plan-draft` is the one exception: `planner check` checks its
form, and a cloud reviewer checks the rest.

Each job has a kind, such as `plan-review` or `local-read`.
`routing.json` in this folder maps each kind to its reasoning level
and an ordered list of models. The list comes in groups. A group
holds models a failed check skips together, such as the two local
models. `scripts/pde_route.py` picks the first model this machine can
run and logs the pick. A pick counts as the user's explicit request
to set a subagent's model.

## Inputs

- The job's kind: the one the skill that owns the job names, else the
  one in Job kinds that fits.
- The tool that will start the agent. It sets `--start`, as Pick the
  model says.

## Pick the model

Run the script with the kind and the start:

```sh
python3 ~/.agents/skills/model-routing/scripts/pde_route.py <kind> --start <start>
```

The start names the tool that starts the agent, not the session's own
harness:

- `opencode-subagent`: OpenCode's `subagent` tool. A Claude Code
  session in OpenChamber starts agents through this tool too.
- `opencode-cli`: `opencode run --standalone -m <model>`, from the
  shell or in a Herdr tab. A Claude Code session runs local jobs this
  way too.
- `claude-agent`: Claude Code's `Agent` tool.
- `claude-cli`: `claude --model`, from the shell or in a Herdr tab.

The script prints one JSON line, such as:

```json
{"model": "github-copilot/claude-opus-5.5#high", "effort": "high", "run_id": "rt-7f3a2c"}
```

Start the agent with that model, and begin its description with the
run ID, such as `rt-7f3a2c plan review`.
Under `claude-cli`, also pass any printed effort as
`claude --effort`. A `-cli` start has no description, so it needs no
run ID. Under `claude-agent` the script prints no effort, since the
`Agent` tool takes none.

1. When the user names a model, pass it with `--model <name>`. The
   script checks that the machine lists it and logs it. When it exits
   1, the machine lacks that model: tell the user, and offer the
   kind's own pick.
2. Otherwise, use the kind the skill that owns the job names.
3. Otherwise, use the kind in Job kinds that fits the job. When no
   kind fits, keep the session's model, or the model the
   configuration sets.
4. Never pick a model by hand, and never move a job to a cheaper
   kind to save tokens.

A skill that still names a role, such as "the review model", means
the kind this list gives. The list goes away when every skill names a
kind:

- local: `local-read`, or `local-script` for a script job and
  `step-code` for code.
- cheap: `cheap-draft`.
- review: `plan-review` for a plan reviewer, `code-review` for a code
  reviewer, such as auto-run's T1 reviewer, and `test-review` for a
  test reviewer.
- strongest: `plan-write`, or `hub` and `orchestrator` for a session
  that directs others.

`plan-draft` has no role, since the skill that starts a planner
names it.

## Job kinds

Each line says which jobs a kind covers. The table gives the models.

- `plan-review`: a review of a plan or design doc.
- `code-review`: a review of a pull request's code, or of a diff.
- `risky-review`: a review that review-plan's rule calls risky, such
  as one of code that can delete the user's files.
- `test-review`, `docs-review`, `comment-review`, `skill-review`: a
  review of tests, source documentation, code comments or skill text.
- `session-check`, `deploy-check`: a check of a session's token use,
  or of a deployed change.
- `pr-text`: pull request text that no stronger model checks.
- `research-read`: reading code or notes when no script checks the
  answer.
- `plan-write`: writing a plan or a decision.
- `plan-draft`: each planner of a plan's first version, when the
  user asked for a local draft for this task or run. A long first
  version may take several planners, each writing the next part.
  Otherwise those planners are `plan-write`. The draft goes to
  review only after `planner check`, which checks the plan's form,
  passes on it. When it fails, give a new planner the errors, and
  retry or move on as When the check fails says. The draft gets
  the same review as any first version. Every later planner, such
  as one for a revision or a fix round, is `plan-write`. Each local
  revision round would cost a full cloud review, and in trials it
  saved the strong planner little work.
- `cheap-draft`: a long draft that a stronger model then checks, or
  sorting by meaning when the session reads each line.
- `web-lookup`: a web lookup after the local model failed it, as Web
  lookups says.
- `step-code`: writing code that tests and linters check. It moves
  from the local model straight to Opus.
- `step-fix`: fixing code that `step-code` could not get right.
- `local-script`: a script job, such as running tests, linters, git
  commands or `planner check`. It also covers a decision-matching
  worker. The user has already said, in plain words, what to do with
  each item in a numbered list, such as "keep 1 to 5, drop 9". The
  worker turns those words into the arguments one script takes, runs
  the script, and reports the counts it prints, such as "5 kept, 1
  dropped". Like a test run, it needs no judgment: the decision is
  made, and the script checks its own input. The report leaves out
  each item's details, such as names and amounts, since the counts
  are enough and the details may be private.
- `local-read`: reading or searching whose answer a script checks,
  such as quoting lines from files or web pages.
- `merge-script`, `merge-session`: a merge by the merge worker script,
  or a merge a cloud session runs from the start.
- `orchestrator`: a worker session that directs other agents. It is
  not a hub.
- `hub`: a Herdr session that other sessions report to.
- `eval-scorer`: the scorer of the skill trials.

A session the user started, such as an auto run's front hub, keeps
the model and level the user started it on. A successor that
`hub launch --successor` starts keeps its caller's model and level,
since it goes on with the same job.

## When a model fails to start

A start fails with a not-found, login or limit error. Run the script
again with the same kind and start, and add `--failed` with the model
that failed:

```sh
python3 ~/.agents/skills/model-routing/scripts/pde_route.py <kind> --start <start> --failed <model>
```

The script prints the next model, which may be the same model at
another provider. You may pass the printed name as it is, with its
`#` suffix. The script keeps no state between calls, so repeat each
earlier `--failed` and `--check-failed` flag of the job. When no model
is left, the script exits 1 and prints each model with why it was
skipped. Stop and report that to the user, or to the session that
gave you the work.

Under a `-cli` start, the script itself sends each model a one-line
job first and skips one that does not answer, so a Herdr tab never
starts on a dead model.

## Brief a local model

1. Write the job as one script. The script sends long output to a log
   and writes a summary file whose last line is `done`.
2. For a script job, give the model no rules file and no skill, only
   this brief:

       Run this one command once, with no time limit on the shell
       tool: `bash <script>`. Then print <summary file> exactly and
       stop. Never run the command again, even when it fails. Touch no
       other file and run nothing else. Never answer an approval
       request.

3. When the job needs edits, such as fixing lint errors, write a
   separate brief. That brief names the input files, the only files
   the model may edit, and the check script the model runs last. Leave
   a check script that commits or pushes out of the brief. You run
   that script yourself, as step 5 says.
4. When the model does the reading or searching, such as quoting lines
   from notes or finding the callers of a function, it runs no
   script. The brief names the input files, the one answer file the
   model writes, and that file's exact format, such as one JSON line
   per match with the file name and an exact quote. The brief also
   says to write nothing else. Pick its model as a `local-read` job:

   ```sh
   python3 ~/.agents/skills/model-routing/scripts/pde_route.py local-read --start <start>
   ```

5. For a script job, read the summary file yourself. The job passes
   its check only when the file's last line is `done`. For an edit
   job, run its check script yourself after the model stops. For a
   reading or search job, run a check script on the answer file
   yourself. When the answer lines quote files or web pages, use
   `scripts/check_quotes.py`. Each line needs `quote` and either
   `file` or `url`. The script writes the lines it found to a table
   file:

   ```sh
   python3 ~/.agents/skills/model-routing/scripts/check_quotes.py \
     <answer file> <table file>
   ```

   Never trust the model's own report on any local job.

## Web lookups

A web lookup that only quotes pages is a `local-read` job. Brief it
as step 4 of Brief a local model says. Use one JSON line per answer,
holding `url` and `quote`. Check it as step 5 says. The script
fetches each page itself.

When the check fails twice, or the local model cannot fetch pages,
run the lookup as a `web-lookup` job:

```sh
python3 ~/.agents/skills/model-routing/scripts/pde_route.py web-lookup --start <start>
```

In OpenCode, also pick the `web-search-researcher` agent.

## When the check fails

The check fails when the summary or answer file is missing, when the
summary's last line is not `done`, or when the check script reports a
wrong result. A summary that ends in `done` and reports failing tests
is a result, not a failed job. Hand that result to the skill that owns
the job.

1. Run the job once more on the same model, with the same brief.
2. When it fails again, run the script again with `--check-failed`
   and that model. It skips every model in that model's group, such
   as the second local model, and prints the first model of the next
   group. Run the job once on it, with the same brief and script:

   ```sh
   python3 ~/.agents/skills/model-routing/scripts/pde_route.py <kind> --start <start> --check-failed <model>
   ```

3. Each time it fails again, add one more `--check-failed` with the
   model that failed, and try once on the printed model. The skill
   that owns the job may name another kind instead, such as a fix
   job.
4. When the script exits 1, stop. Report the job, the check and the
   last summary to the user, or to the session that gave you the
   work.

## Check the machine and the log

- `pde_route.py check` lists each table model this machine lacks. It
  exits 1 when some kind has no model here. The installer runs it.
- `pde_route.py audit --since 1d` pairs each subagent start with its
  pick by run ID. It names each start with no pick, no model or
  another model, and exits 1 when it finds one.

## Output format

Each report on a job names the model that ran it. When that is not
the kind's first model, the report also says why, as the script's log
gives it, such as "not on this machine".

## Guardrails

- A local model writes no review, decision or pull request text.
  It writes a plan only as a `plan-draft` job, as Job kinds says.
- Never pick a model by hand. A job's model comes from the user, a
  rule such as one in `ai/AGENTS.md`, or the kind a skill names.
- Never pass a weaker model as Claude Code's `--fallback-model`.
- A kind a skill names for one of its own jobs wins over the kind
  Job kinds gives.

## When a tool is missing

- If the script exits 2 with "cannot read the model list", report
  that `opencode models` failed. Do not start the agent on a guessed
  model.
- If `python3` does not run, keep the session's model and say you
  could not pick one.
- If a local job's model stops answering partway, count it as a
  failed check.

## Final checks

- Each agent started on the model the script printed, with the run ID
  first in its description.
- Each failed start ran the script again with `--failed`, and each
  failed check with `--check-failed`, repeating the earlier flags.
- Each local job passed its check. A script job's summary file ends
  in `done`. For an edit job or a reading or search job, its check
  script passed when you ran it.
- You read the summary file or ran the check script yourself.
- No local model wrote a review, decision or pull request text, or
  any plan version other than a `plan-draft` first version.

## References

- [references/evaluation.md](references/evaluation.md): read only
  when testing or revising this skill.

## Remembering corrections

Follow every lesson in the Lessons section below. Each is a correction
the user made, so a lesson wins when it disagrees with the text above.
When the user corrects which model runs a job or how a local job is
checked, follow [skill-fix](../skill-fix/SKILL.md) with the skill name
`model-routing`, and offer once to fix the skill's text.

## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
