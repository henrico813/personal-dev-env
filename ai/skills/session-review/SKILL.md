---
name: session-review
description: Reviews an OpenCode session for token use, corrections
  worth saving as skill lessons, and work a local model could take.
  A script reads the session's numbers from the OpenCode database, so
  the review spends tokens only on judgment. Proposes at most three
  changes and changes nothing until the user picks. Use when the user
  asks to review this session for token usage, efficiency, skill
  lessons or local model handoff.
metadata:
  pde-workflow: "true"
---

# Session Review

The user often ends a long session by asking what made it costly,
which corrections should become lessons, and what a local model could
have done. A script prints the session's numbers in about 20 lines.
You judge from those lines and the conversation, and propose a few
changes the user can keep for good.

## Inputs

- The session: the current one, or an ID the user names.
- This conversation, for the user's corrections and decisions.
- Other sessions, only when the user asks for lessons across them.

## Steps

### 1. Collect the numbers

Run the script, read-only, from anywhere:

    python3 ~/.agents/skills/session-review/scripts/session_stats.py

It reads the session in `$OPENCODE_SESSION_ID`. In a subagent that
variable holds the subagent's own ID, so pass the parent's ID with
`--session <ID>`.

The script prints totals, user messages and model steps, the peak
context, compactions, each skill with the step that first loaded it,
subagent tokens by model with the five largest subagents, and
commands run more than once with the same text. The peak is
approximate, because the claude-code provider logs some steps with
zero tokens and adds them to the next step; the script leaves that
next step out.

Query the database yourself only for a fact the script does not
print. Open it read-only with `python3` (there is no `sqlite3`
binary), and name the query in the review. Never report dollars:
`cost` is 0 for claude-code sessions.

If the script exits non-zero, review from the conversation alone and
say so at the top of the review and the message.

### 2. Judge

Write at most three proposals, ranked by saving. Each one says:

- What changes, and where it would live: a skill pull request, a
  skill lesson, a memory, or a rules file such as `AGENTS.md`.
- The saving, measured from the script's lines or marked
  "estimate" with its arithmetic. For example, 30,000 tokens of
  output read again by 80 later steps is about 2.4M tokens.
- Why it is worth keeping long term: it would help a different
  session next month. A one-time slip fails this test.

Drop any proposal on the Do not propose list below.

### 3. Lessons

List the corrections the user made in this session as a numbered
pick list. Keep only those that pass step 3 of
[skill-lessons](../skill-lessons/SKILL.md), and name the skill each
would go to. Mine other sessions only when the user asks, and only
the sessions they name; mining a day of sessions has cost over 13M
tokens.

### 4. Local models

The user wants local models able to run the whole workflow,
including the hub, its workers and session reviews, if cloud models
become unavailable.
While cloud models are available, local models should do work that
cuts cloud tokens. Answer both in two required sections, for each
role the session used, workers as well as the hub. A list of jobs a
script could check is not enough.

- Fallback map: for each role, such as hub, planner, implementer,
  reviewer or merge worker, the local model that would run it, what
  breaks, what would make it work, and the evidence, a measurement
  or a trial.
- Efficiency map: each large reading, sorting, summarizing or
  checking step a local model could do now, so the cloud model reads
  a short result. Give its tokens from the script or a named query,
  the check that makes the local result safe to trust, and the
  saving, marked measured or estimate.

Find a trial in an earlier review in the session-reviews folder whose
role this session used. Rerun it on the same local model, and give
its result in a Trials section. If you ran none, say so there and
why.

At least one of step 2's proposals, or one lesson, helps a local
model take over a workflow. It may use cloud and local models
together, or a script that a local agent can run and that saves a
cloud agent tokens. Proposals from the two maps count toward step
2's limit of three.

To brief a local model on a job a script checks, point to the Local
model section of
[roadmap-run's prompts](../roadmap-run/references/prompts.md) rather
than restating it. Keep cheap cloud models, such as
`openai/gpt-6-luna`, apart from local ones, such as `goog/qwen3.8`:
only a local model keeps working when cloud access fails.
For long, repetitive work, such as an audit that repeats one check
for each ticket, follow
[herdr-handoff's page](../herdr-handoff/references/cheaper-model.md)
to prepare it for a cheaper model.

### 5. Write the review and have it checked

1. Save the review as
   `~/.local/state/pde/session-reviews/<date>-<slug>.md`. Get the
   date with `date +%Y%m%d`. Use the sections in
   [references/example.md](references/example.md).
2. Start one fresh subagent. Give it the review file's path and the
   session ID. It reruns the script, redoes each proposal's
   arithmetic, runs only the queries the review names, and checks
   the proposals against the Do not propose list. It never reads
   message bodies and changes no file.
3. Fix or drop what it rejects. If you cannot start a subagent,
   check the review yourself and say so in the message.

### 6. Report

Send one short message by the `status-updates` skill, in the format
below. Then wait. Change nothing until the user answers.

A narrow question, such as "what could a local model have done?",
still gets the Fallback, Efficiency and Trials lines at the end. A
trial is a rerun from step 4, not work the session already did on a
local model. It also keeps the line that points to roadmap-run's
Local model section.

## Output format

    Session review of <title>, saved to <review file path>.

    <one line: tokens read, peak context, steps>

    Proposals
    1. <change> in <where it lives>. Saves <n> tokens (measured).
    2. ...

    Lessons to save, pick by number
    3. <rule> -> <skill>

    Fallback: <the role least ready for a local model, and its fix>.
    Efficiency: <the largest step a local model could take, and its
    saving>.
    Trials: <the trial you reran and its result, or "none ran">.
    To brief a local model, see the Local model section of
    roadmap-run's references/prompts.md.

    Reply with the numbers to apply.

Proposals and lessons share one run of numbers through the whole
review, so each number in a reply like "1, 4" points to a single item.

## Guardrails

- Change nothing, including skills, lessons, memories and rules
  files, until the user picks.
- Edit a shared rules file only when this session owns it or the
  user says so. Otherwise send the change to its owner, such as the
  session that coordinates the run.
- Never report dollars or a number the script or a named query did
  not give.
- Never copy a secret from tool output into the review.

## Do not propose

- Skipping a skill that applies to the work, to save tokens. Loading
  a skill when the step that needs it comes, as a handoff may say, is
  fine; leaving it out is not.

When the user rejects a proposal as a kind they never want, offer
once, by [skill-lessons](../skill-lessons/SKILL.md), to save it as a
lesson that starts "Do not propose". The lesson then holds in every
review, and the user can move it into this list by a pull request.

## When a tool is missing

- The script fails or the database is missing: review from the
  conversation alone and say so.
- No subagent tool: check the review yourself and say so.
- `skill-lessons` is not installed: list the lessons and offer
  nothing to save.

## Final checks

- The review's Numbers section matches the script's output.
- At most three proposals, each with a saving marked measured or
  estimate, and where it would live.
- No proposal is on the Do not propose list or a lesson.
- The review has a Fallback map, an Efficiency map and a Trials
  section, covering every role the session used.
- A proposal or a lesson helps a local model take over a workflow,
  as step 4 asks.
- No dollars.
- The fresh check ran, or the message says it did not.
- Nothing was changed.

## References

- [references/example.md](references/example.md): read before
  writing the review file.
- [references/evaluation.md](references/evaluation.md): read only
  when testing or revising this skill.

## Remembering corrections

Follow every lesson in the Lessons section below. Each is a correction
the user made, so a lesson wins when it disagrees with the text above.
When the user corrects how you review a session or rejects a kind of
proposal, follow [skill-lessons](../skill-lessons/SKILL.md) with the
skill name `session-review`, and offer once to save the correction as
a lesson.

## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
