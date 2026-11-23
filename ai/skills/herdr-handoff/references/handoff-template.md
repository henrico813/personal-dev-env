# Handoff Template

Copy the outline in the code block below into the handoff file and
fill each part. Leave out a part with nothing in it. Keep the reader
in mind: a new session that knows nothing of this one, and may reread
the file after compaction. In the outline, text in `<...>` speaks to
you, the writer, and calls the reader "the new session". Text outside
it speaks to the new session as "you". One exception: leave `<file>`
in the `hub report` bullet as it is. The new session puts its report
file there each time.

## Contents

The outline's parts, in order: the title and opening lines, The
project, What happened, What the user decided, Your work, Questions
for the user, Coordination, Rules for the files you edit, and
Appendix.

```markdown
# Handoff: <the work, in a few words>

You are a new session. The user asked for this work in a new Herdr
workspace or tab. Another session ("<its name>", Herdr workspace or
tab `<label>`) wrote this handoff and keeps the work listed under
Coordination.

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

1. <Each decision, in the user's words where the wording matters.
   For an answer to a question, add where the user gave it and the
   question's Q number, such as "(Q3)".>

## Your work

### 1. <First task>

<What to do, what to propose to the user before acting, and how to
check it. Back up anything before replacing it.>

### 2. <Next task>

Order: <which task comes first and why, and what may be drafted while
waiting on the user.>

## Questions for the user

<Each open question word for word, with its Q number, its choices, the
choice this session recommends and why, and its `Open:` line, as step
4 of "Write the handoff file" in SKILL.md says. Name where the user
may have answered each one, such as a session's pane or a card.>

Before you ask a question below, look for the user's reply in each
place it names and in the card's Decisions. Each question's `Open:`
line names the places the last session checked, and when. In those
places, read only what came after that time. Read every other place
in full, and a place that shows no times too, since you cannot tell
what is new there. Drop each question the user answered or made moot.
A plain reply can settle a question, as "Questions for the user" in
the `herdr-hub` skill says. Ask the rest first, with the
same Q numbers, in the short form of the `status-updates` skill,
since the user's replies name questions by these numbers. Give no Q
number listed in What the user decided to a new question until its
batch ends, as "Question numbers" in that skill says. When this
handoff names a report file, write the questions there in full
instead, as Coordination says.

## Coordination

- Read: <each file the new session may read, by absolute path.>
- Edit: <each file it may edit, by absolute path. No other session
  edits these files.>
- <Which session owns each file, branch and worktree.>
- **<What the new session must not touch, until when.>**
- <Who records ownership changes on the tracking page, if any.>
- Write progress-only news to <the new session's status file>.
  Report each question, blocker or finished work that "<this
  session's agent name>" waits on, such as a PR opening. Run
  `hub report --run "<run folder>" <the new session's agent name>
  <file>`, then end your turn. The command replaces <report file>
  whole with <file>. Never prompt "<this session's agent name>"; it
  learns of your report when your turn ends.
- <When this session keeps nothing and no hub runs: leave out the
  `hub report` bullet above, and say the new session asks the user in
  its own pane.>
- <When a hub runs, change three things in the `hub report` bullet
  above. Put the hub's agent name in place of "<this session's agent
  name>". Put the hub's run folder in place of <run folder>. Put a
  file in that folder in place of <report file>. Then add the bullet
  "You report only to the hub." to the handoff. Below it, turn the
  rest of this placeholder into bullets. Each bullet speaks to the
  new session as "you". To wait on a long job, such as a test run,
  the new session first runs `hub wait` with
  `--run "<run folder>"`, its agent name, `--file <summary file>` and
  `--deadline <minutes>`. Then it starts the job with
  `setsid nohup bash <script> > <log> 2>&1 &`, so the job outlives
  the tool call, and ends its turn. The script writes the summary
  file in one step at its end. The hub's watch prods the new session
  when that file changes or the deadline passes. The new session never
  uses an interactive question tool, since Herdr then rejects every
  message to it. A message that starts with `hub: ` comes from the
  hub; any other message in its pane comes from the user. When the
  user answers a question or decides something in its pane, it
  reports that to the hub at once, since the hub's question list never
  sees the pane. It sends a pull request edit request only after the
  hub tells it the user reviewed that pull request, and a merge request
  only after the hub tells it the user asked to merge it, as
  `ai/AGENTS.md` says. When its
  card reaches the `done` or `dropped` stage, or once it has written a
  handoff that pauses the card, it checks that it holds nothing unsaved,
  such as uncommitted files or an unpushed branch, or, for a pause, that
  its handoff lists each one. It never closes its own tab; the hub
  closes or parks it, and closes it after a pause once it has read that
  report.>

## Rules for the files you edit

<Rules the new session cannot guess, such as a note format, a tool to
use or avoid, or a line width.>

## Appendix: <results that live only in this conversation>

<Subagent reports, review findings and other results the new session
cannot see anywhere else. Copy them in full.>
```

Before saving, check that the file names every skill the work needs,
each with the step that needs it.
