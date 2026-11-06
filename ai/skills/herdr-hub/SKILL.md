---
name: herdr-hub
description: Runs the hub, the one Herdr session that other sessions
  in a long run report to. The hub keeps the list of sessions and what
  each owns, passes messages, answers questions from the user's
  decisions, and runs every merge through a subagent. Use when the
  user or a handoff makes this session the hub of several Herdr
  sessions. To start one new session, use herdr-handoff; to build a
  roadmap's pull requests, use roadmap-run.
metadata:
  pde-workflow: "true"
---

# Herdr Hub

The hub is the one Herdr session that the other sessions of a run
report to. Each other session reports only to the hub, through a
report file, and never prompts it. The hub tells
the user only what the user must do, or what changes what the user
sees. Status-only news waits for the next real update. For the hub,
this rule wins over the line of `status-updates` that asks for an
update after each step of delegated work.

Load `status-updates` at the start. Load each other skill only when
its step comes, such as `herdr-handoff` when you start a session.

## The session list

The hub's run folder is the one its handoff names. Without one, make
it at the start with `mktemp -d ~/.local/state/herdr-hub.XXXXXX`.
Its `status.txt` is the hub's own file, separate from the run folder
and `status.txt` of any roadmap-run pull request.

The hub's `status.txt` is the one current list of sessions. Give each
session one line: agent name, workspace, what it owns, and handoff
path. Keep the file to about 20 lines. Record the user's standing
decisions there, or in the run's rules file when they do not fit. A
tracking page, such as a roadmap note, keeps its dated ownership
notes as the user's log.

Before you rely on a session, or promise the user something from it,
check that it still runs. No output means it is gone. An error from
`herdr` means the check failed, not that the session is gone, except
`agent_not_found`, which means it is gone. Never print the whole
list.

    herdr agent list | jq -r '.result.agents[]
      | select(.name == "<name>") | .agent_status'

When you hand work to a new session with `herdr-handoff`, its
Coordination section names you as the one session it reports to,
gives its report file, such as `<run folder>/report-<name>.md`, and
names no other owner. Workers never message each other. Add the new
session's line to `status.txt`.

## Watch each session

A session writes its report and ends its turn. Learn of it with one
background command per session, started once the session works:

    herdr agent wait <name> --until idle --until done \
      --until blocked >"<run folder>/watch-<name>.txt" 2>&1

When the wait returns at `blocked`, read the pane as Send to a
session says. Otherwise read the report file, handle it as Reports
from sessions says, then rename it to `report-<name>.read`. No report
file means the turn ended with no news. Never start a watch on an
`idle` or `done` session, since it returns at once. Such a session
waits for your next message, and `send.sh` watches that turn. After
a watch returns at `blocked`, watch with only `--until idle` and
`--until done`, since a `blocked` session matches `--until blocked`
at once.

A session started with `herdr --machine <label>` runs on that
machine's Herdr server, and you cannot read its files. Write in its
handoff that it ends each turn with its report. Watch it with the
command above, with `--machine <label>` after `herdr`. Read the
report with the first command below. To check that the session still
runs, pipe the second command into the `jq` filter above.

    herdr --machine <label> agent read <name> \
      --source recent-unwrapped --lines 80
    herdr --machine <label> agent list

## Reports from sessions

- A question: answer it yourself when the user's standing decisions,
  the repository's usual practice or the run's design document
  settles it. Bring the user only a question whose answer changes
  what the user sees, with your recommendation.
- Finished work, such as an open pull request: record it in
  `status.txt`, then start the next step.
- Status-only news: record it in one line of `status.txt`. Tell the
  user nothing about it.
- A change of owner: update `status.txt`, and tell each session whose
  work changes.

## Send to a session

Send every message, such as an answer, with this skill's
`scripts/send.sh`, run in the background. Start it at once, even when
the session is busy or `blocked`:

    bash <this skill's folder>/scripts/send.sh <name> \
      "<run folder>" "<message>"

For a session on another machine, put `--machine <label>` before
`<name>`. The script waits until the session is `idle` or `done`,
since `herdr agent prompt` rejects every message to a `blocked`
session. It also waits while the user is in the session's pane. Then
it sends the message and waits until the session ends that turn, so
it also watches that turn. A busy session gets the message after its
current turn, up to 30 minutes later.

It writes to `<run folder>/send-<name>.txt`, which ends with an
`exit` line when the send is over:

- No `exit` line: the send is pending. Never send the message again
  by hand, and never start a second send to that session. The script
  refuses one with exit code 2.
- `exit 0`: read the state with `herdr agent get <name>`, and handle
  it as a returned watch.
- Any other `exit` line: the message may not have been sent. Check the
  session with `herdr agent get <name>` before sending again.
- No file: the script never ran, so run it once more. If your harness
  refuses it again, tell the user.

If the file still has no `exit` line when `herdr agent get <name>`
shows the session idle with `.focused` false, read the file once
more. Then tell the user the message may not have been sent.

When a session is `blocked`, read what it asks with
`herdr agent read <name> --source recent-unwrapped --lines 40`. Tell
the user once which session waits, what it asks, and your
recommendation. Never answer it yourself.

## Merges

The hub runs every merge, in a background subagent that starts with a
small context. Load `pull-request` yourself, then brief the subagent
to load it and run Merge steps 1 to 3 and Guardrail 4. It writes the
results to files in a new folder and reports in at most 20 lines.
Show the user what Merge step 4 lists, from those files. Then have
the same subagent run steps 5 and 6.

## Rules

- Never answer an approval or question shown in another session's
  pane, run `pde-pr-approve`, or set `PDE_GH_WRITE`.
- Never close a workspace or agent. They belong to the user.
- Write each update to the user by `status-updates`.
- When your context grows past about 150k tokens, or after it is
  summarized, use the `herdr-handoff` skill to offer the user a new
  session. Offer once.

Read [references/evaluation.md](references/evaluation.md) only when
testing or revising this skill.

## Remembering corrections

Follow every lesson in the Lessons section below. Each is a correction
the user made, so a lesson wins when it disagrees with the text above,
except that you still never answer an approval or question shown in
another session's pane. When the user corrects how you run the hub,
pass messages or merge, follow
[skill-lessons](../skill-lessons/SKILL.md) with the skill name
`herdr-hub`, and offer once to save the correction as a lesson.

## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
