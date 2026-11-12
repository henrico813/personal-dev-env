---
name: model-routing
description: Holds the one list of models, and says which model runs
  each kind of agent job, how to check that a model is available, how
  to brief a local model, and what to do when a job fails its check.
  Use when choosing the model for a subagent, a new Herdr session, a
  worker or a job script, when the user's chosen model cannot run,
  and when another skill points here.
---

# Model Routing

Each agent job runs on the weakest model whose result you can trust.
A local model costs no cloud tokens and still runs when cloud access
fails. You cannot trust a local model's own report on a job, though.
So a job goes to a local model only when a script checks its result.

This skill holds the only list of models. Other skills name a role
only, such as "the local model, as `model-routing` picks". A choice
made here counts as the user's explicit request to set a subagent's
model.

## Inputs

- The job: what the agent does and what it hands back.
- The harness that starts the job: OpenCode or Claude Code.
- In OpenCode, the models on the machine that runs the job, from
  `opencode models`. Names differ between machines, so take each
  exact name from that list.

## Roles

Each job has one role, set by what the job needs. The roles run from
weakest to strongest.

1. Local: a script checks the result. Examples are running tests,
   linters, git commands, merge checks or `planner check`, quoting
   lines that a script finds in their source file, and applying a
   plan step that tests check. The model only has to follow a short
   brief and call tools. Such a job is a local job.
2. Cheap: either the job needs little judgment, or a stronger model
   or the session checks the output. Examples are web lookups that only
   quote pages, long prose drafts that a stronger model then checks,
   and sorting or labelling by meaning when the session reads each
   line.
3. Review: the job needs judgment over a bounded input. Its model
   runs at medium reasoning. Examples are reviews of plans, diffs,
   comments and tests, including `code-documentation` and
   `behavior-focused-testing` reviews. Writing pull request text, and
   writing code outside a checked plan step, are review jobs too.
4. Strongest: the job needs the deepest reasoning and a long context,
   up to one million tokens for a large plan. Examples are plans,
   decisions, risky reviews, the hub of other sessions, and the last
   try of any failed job. A review is risky when the code it checks
   can change or delete the user's files or data. A skill may also
   mark its own reviews as risky.

## Models

Each role has an ordered list of models for each harness. The first
name is the default, and later names are fallbacks.

OpenCode, for `--model` and the subagent tool's `model`:

- Local: `goog/qwen3.8`.
- Cheap: `openai/gpt-6-luna`, `opencode-go/gpt-6-luna`,
  `claude-code/claude-haiku-4-5-20251001`.
- Review: `claude-code/claude-sonnet-5`,
  `github-copilot/claude-sonnet-5`.
- Strongest: `claude-code/claude-opus-5-5[1m]`. The `[1m]` marks the
  one-million-token version. Use high reasoning where the harness
  takes it.

Adding `#medium` to a name, as in
`claude-code/claude-sonnet-5#medium`, sets medium reasoning where the
tool takes a variant. The availability check compares the name
without it.

Claude Code, for a subagent's `model` and `claude --model`:

- Local: the subagent tool cannot reach a local model. Run a local
  job from the shell with `opencode run --standalone -m <model>`,
  with the model from the OpenCode list.
- Cheap: `haiku`.
- Review: `sonnet`.
- Strongest: `opus`.

Each Claude Code alias follows the newest model of its kind.

## Check that a model is available

- OpenCode: the name must be in `opencode models` on the machine that
  runs the job. Before you start a session or worker on a model, send
  that model a one-line job. When it does not answer, skip it and try
  the next name:

  ```sh
  opencode run --standalone -m <model> "reply OK"
  ```

- Claude Code: no command lists the models. Each alias always
  resolves to a current model. Count a model as unavailable when a
  call fails with a not-found, login or limit error.

## Pick the model

1. Use the model the user asks for. When it is unavailable, offer the
   first available model of the job's role instead. For work a script
   checks, such as a plan whose steps all have tests, that first model
   is the local model. The local model runs even while the cloud is
   down.
2. Otherwise, use the role a skill names for one of its own jobs. The
   skill, or one of its lessons, may also set the job's reasoning
   level.
3. Otherwise, use the job's role from Roles.
4. Take the first available model in that role's list. A web lookup
   is the one exception, as Web lookups says.
5. When no model of the role is available, step up to the next
   stronger role. Never step down. When no strongest model is
   available, stop and report to the user, or to the session that
   gave you the work.

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
   the model may edit, and the check script the model runs last.
4. When the model does the reading or searching, such as quoting lines
   from notes or finding the callers of a function, it runs no
   script. The brief names the input files, the one answer file the
   model writes, and that file's exact format, such as one JSON line
   per match with the file name and an exact quote. The brief also
   says to write nothing else.
5. For a script job, read the summary file yourself. The job passes
   its check only when the file's last line is `done`. For an edit
   job, run its check script yourself after the model stops. For a
   reading or search job, run a check script on the answer file
   yourself, such as one that finds each quote in its source file. A
   quote often joins wrapped lines, so the check collapses runs of
   whitespace in the quote and the source before it compares them.
   Never trust the model's own report on any local job.

## Web lookups

A subagent that only searches the web and quotes pages has the cheap
role. It is the one exception to taking the first model in the list.
Pick the cheap model from the provider of the session's own model,
since that provider is known to work. Set the subagent tool's `model`
to it. In OpenCode, also pick the `web-search-researcher` agent. If
the tool cannot take that model, or the call fails, run the lookup
without setting a model.

## When the check fails

The check fails when the summary or answer file is missing, when the
summary's last line is not `done`, or when the check script reports a
wrong result. A summary that ends in `done` and reports failing tests
is a result, not a failed job. Hand that result to the skill that owns
the job.

1. Run the job once more on the same model, with the same brief.
2. When it fails again, step up one role and run it once, with the
   same brief and script. The skill that owns the job may give a
   stronger model other work instead, such as fixing the job.
3. Each time it fails again, step up one more role and try once,
   until the strongest model has tried. A job that starts on the
   local model can try the local, cheap, review and strongest models.
4. When the strongest model fails too, stop. Report the job, the check
   and the last summary to the user, or to the session that gave you
   the work.

## Output format

Each report on a job names the model that ran it. When that is not
the role's default, the report also says why, such as "the default
did not answer".

## Guardrails

- A local model writes no plan, review, decision or pull request
  text.
- Never step a job down to a weaker role, even when its model is
  unavailable.
- Never pass a weaker model as Claude Code's `--fallback-model`.
- A role a skill names for one of its own jobs wins over the role
  here.

## When a tool is missing

- If `opencode models` does not run, use the names in Models and say
  you could not check them.
- If a model does not answer, use the next name in its role's list.
  When the list runs out, step up a role as Pick the model says.
- If a local job's model stops answering partway, count it as a
  failed check.

## Final checks

- Each job ran on the first available model of its role, or its
  report says why not. A web lookup ran on the cheap model of the
  session's own provider.
- No job moved to a weaker role.
- Each local job passed its check. A script job's summary file ends
  in `done`. For an edit job or a reading or search job, its check
  script passed when you ran it.
- You read the summary file or ran the check script yourself.
- A job that failed its check twice stepped up one role at a time,
  with one try on each. The skill that owns the job may instead have
  given a stronger model other work. Either way, the job stopped after
  the strongest model.
- No local model wrote a plan, review, decision or pull request text.

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
