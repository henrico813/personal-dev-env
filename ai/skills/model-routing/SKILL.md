---
name: model-routing
description: Says which model runs each kind of agent job, how to
  brief a local model, and what to do when a local job fails its
  script check. Use when choosing the model for a subagent, a new
  Herdr session, a worker or a job script, and when another skill
  points here for local model work.
---

# Model Routing

Each agent job runs on the cheapest model whose result you can trust.
A local model costs no cloud tokens and keeps working when cloud
access fails. Its own report on a job cannot be trusted, though. So a
job goes to a local model only when a script checks its result.

## Inputs

- The job: what the agent does and what it hands back.
- The models on this machine, from `opencode models`. Names differ
  between machines, so take each exact name from that list. The names
  below are this setup's defaults. When a default is missing from the
  list, use the closest model of the same kind, such as another local
  model, and say which one you used.

## Pick the model

1. Use the model the user names.
2. Otherwise, use the model a skill names for one of its own jobs.
   For example, roadmap-run says who writes pull request text. The
   steps of a plan follow implement-plan's `references/build.md`,
   which picks their models and says what to do when a step fails.
3. Otherwise, pick by the job:
   - Local, `goog/qwen3.8`: git commands, merge checks, worktree
     cleanup, tests, linters, code search and reading files.
   - Cheap cloud, `openai/gpt-6-luna`: long prose drafts, such as a
     design doc draft or notes. Opus then checks the draft.
   - Opus, `claude-code/claude-opus-5-5[1m]`: plans, reviews,
     decisions, and the hub of other sessions. Type the name as
     written, since `[1m]` marks the version with a one-million-token
     context.
   - Web lookups that only quote pages: the small model that
     `ai/AGENTS.md` names under Subagent Models.
4. Send a job to the local model only when a script can check its
   result. Tests that pass, a quote found in its source file,
   `planner check` passing and a summary whose last line is `done`
   are such checks.
5. Keep sorting or labelling by meaning on a cloud model, since no
   script can check it. In one trial, a local model sorted five tasks
   by the machine they run on and got none right.

## Brief a local model

1. Write the job as one script. The script sends long output to a log
   and writes a summary file whose last line is `done`.
2. Give the model no rules file and no skill, only this brief:

       Run this one command: `bash <script>`. Then print <summary file>
       exactly. Touch no other file and run nothing else. Never answer
       an approval request.

3. When the job needs edits, such as fixing lint errors, the brief
   also names the input files, the only files it may edit, and the
   check script it runs last.
4. When the model reads or searches itself, such as quoting lines
   from notes or finding the callers of a function, it runs no
   script. The brief names the input files, the one answer file the
   model writes, and that file's exact format, such as one JSON line
   per find with the file name and an exact quote. The brief also
   says to write nothing else.
5. For a script job, read the summary file yourself. The job counts
   only when its last line is `done`. For a reading or search job,
   run a check script on the answer file yourself, such as one that
   finds each quote in its source file. A quote often joins wrapped
   lines, so the check collapses runs of whitespace in the quote and
   the source before it compares them. Never trust the model's own
   words about either.

## When the check fails

The check fails when the summary or answer file is missing, when the
summary's last line is not `done`, or when the check script reports a
wrong result. A summary that ends in `done` and reports failing tests
is a result, not a failed job. Hand that result to the skill that owns
the job.

1. Run the job once more on the local model, with the same brief.
2. When it fails again, run it once on the cheap cloud model,
   `openai/gpt-6-luna`, with the same brief and script.
3. When that fails too, run it once on Opus, with the same brief and
   script.
4. When Opus fails too, stop. Report the job, the check and the last
   summary to the user, or to the session that gave you the work.

## Guardrails

- A local model writes no plan, review, decision or pull request
  text.
- Where a skill names its own model for a job, that skill wins.

## When a tool is missing

- If `opencode models` does not run, use the names above and say you
  could not check them.
- If the local model does not answer, count it as a failed check and
  go to step 2 of When the check fails, the cheap cloud model.

## Final checks

- Each local job has a check script, and a summary file whose last
  line is `done` or an answer file the check script read.
- You read the summary file or ran the check script yourself.
- A job that failed twice on the local model ran once on the cheap
  cloud model, and once on Opus only when that failed too.

## References

- [references/evaluation.md](references/evaluation.md): read only
  when testing or revising this skill.

## Remembering corrections

Follow every lesson in the Lessons section below. Each is a correction
the user made, so a lesson wins when it disagrees with the text above.
When the user corrects which model runs a job or how a local job is
checked, follow [skill-lessons](../skill-lessons/SKILL.md) with the
skill name `model-routing`, and offer once to save the correction as
a lesson.

## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
