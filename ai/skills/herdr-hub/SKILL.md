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
report to. Each other session messages only the hub. The hub tells
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
`herdr` means the check failed, not that the session is gone. Never
print the whole list.

    herdr agent list | jq -r '.result.agents[]
      | select(.name == "<name>") | .agent_status'

When you hand work to a new session with `herdr-handoff`, its
Coordination section names you as the one session to message, and
names no other owner. Workers never message each other. Add the new
session's line to `status.txt`.

A session started with `herdr --machine <label>` runs on that
machine's Herdr server. Do not count on it reaching you with
`herdr agent prompt`. Write in its handoff that it ends each turn with
its report instead. Read the report with the first command below. To
check that the session still runs, pipe the second command into the
`jq` filter above.

    herdr --machine <label> agent read <name> \
      --source recent-unwrapped --lines 80
    herdr --machine <label> agent list

## Messages from sessions

- A question: answer it from the user's standing decisions. Bring the
  user only a question whose answer changes what the user sees, with
  your recommendation.
- Finished work, such as an open pull request: record it in
  `status.txt`, then start the next step.
- Status-only news: record it in one line of `status.txt`. Tell the
  user nothing about it.
- A change of owner: update `status.txt`, and tell each session whose
  work changes.

## Send to a busy or blocked session

A session that waits at a question or approval shows as `blocked`.
`herdr agent prompt` rejects every message to it with
`agent_blocked`. Wait for it, then send once, in one background
command that writes to a file in the run folder:

    { herdr agent wait <name> --until idle --until done \
        --until working --timeout 1800000 &&
      herdr agent prompt <name> "<message>"; echo "exit $?"; } \
      >"<run folder>/send-<name>.txt" 2>&1

Keep the three `--until` flags. Plain `herdr agent wait` also stops
at `blocked`, so it returns at once. The send is pending while the
file has no `exit` line. Never send the message again by hand while
it is pending. A nonzero `exit` line means the message was not sent.
Check the session with `herdr agent get <name>` before sending again.
If the file does not exist, the command never ran, so run the same
command once more. If your harness refuses it again, tell the user.
If the file exists with no `exit` line, the command ran, so never
start a second wait for the same message. If the file still has no
`exit` line when `herdr agent get <name>` shows the session idle,
read the file once more. Then tell the user the message may not have
been sent.

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
