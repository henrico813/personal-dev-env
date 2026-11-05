# Handoff Template

Copy this outline into the handoff file and fill each part. Leave out
a part with nothing in it. Keep the reader in mind: a new session that
knows nothing of this one, and may reread the file after compaction.

```markdown
# Handoff: <the work, in a few words>

You are a new session. The user asked for this work in a new Herdr
workspace. Another session ("<its name>", Herdr workspace `<label>`)
wrote this handoff and keeps the work listed under Coordination.

Load `<skill the first step needs>` now. Load `<skill>` before <the
step that needs it>. Load nothing else up front: every loaded skill
is re-read on each turn.

## The project

<What it is and does, in a few lines. Absolute paths to the
repository, design, plans and tracking page, and what state each is
in.>

## What happened

<What was done so far, and what went wrong, with its causes. Enough
that the new session does not repeat a mistake.>

## What the user decided

1. <Each decision, in the user's words where the wording matters.>

## Your work

### 1. <First task>

<What to do, what to propose to the user before acting, and how to
check it. Back up anything before replacing it.>

### 2. <Next task>

Order: <which task comes first and why, and what may be drafted while
waiting on the user.>

## Questions for the user

<Each open question, the choices, and this session's proposal. The
new session asks these first. When a hub runs, the new session
sends them to the hub instead.>

## Coordination

- <Which session owns each file, branch and worktree.>
- **<What the new session must not touch, until when.>**
- <Who records ownership changes on the tracking page, if any.>
- Write progress-only news to <your status file>. Prompt
  "<this session's agent name>" with a question, a blocker, or
  finished work it waits on, such as a PR opening or a step finishing.
- <When a hub runs: the hub's agent name, in place of this session's
  name in the line above, and that the new session messages only the
  hub. The new session never uses an interactive question tool, since
  Herdr then rejects every message to it.>

## Rules for the files you edit

<Rules the new session cannot guess, such as a note format, a tool to
use or avoid, or a line width.>

## Appendix: <results that live only in this conversation>

<Subagent reports, review findings and other results the new session
cannot see anywhere else. Copy them in full.>
```

Before saving, check that the file names every skill the work needs,
each with the step that needs it.
