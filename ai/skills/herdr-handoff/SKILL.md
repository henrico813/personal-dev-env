---
name: herdr-handoff
description: Hands part or all of a session's work to a new agent
  session in a new Herdr workspace. A handoff file carries everything
  the new session needs, since it starts with no earlier context. Use
  when the user asks to hand work off to a new session, to start a new
  session in a new Herdr workspace, or to move work out because it is
  independent or this session's context is full. For a subagent whose
  work the session reviews, use delegate.
metadata:
  pde-workflow: "true"
---

# Herdr Handoff

Herdr is a terminal workspace manager with workspaces, tabs and panes,
and a `herdr` CLI that starts and watches coding agents. This skill
covers handing work to a new agent session in a new Herdr workspace.
The new session remembers nothing of this one, so a handoff file
gives it everything it needs.

For command syntax, read Herdr's own skill with `herdr --skill`, but
only after the check in step 1 of Start the session passes. The
installed binary is the authority, so check `herdr <group> --help`
rather than trusting the examples here.

## When to hand off

- The user asks for it.
- Part of the work is independent of yours and can run alongside it.
- Your context is nearly full and the rest needs a fresh start.

When the user has not asked, propose the handoff and wait for a yes.
Herdr's skill says to create a workspace only when the user asks.

## Write the handoff file

1. Save it as `~/.local/state/pde/handoffs/<date>-<slug>.md`, such as
   `20261101-design-process.md`. Get the date with `date +%Y%m%d`.
   Never put it in the repository, a pull request or the vault.
2. Use the sections and order in
   [references/handoff-template.md](references/handoff-template.md).
3. Copy into it every result that lives only in this conversation,
   such as subagent reports and review findings. The new session
   cannot see them, so they are lost otherwise.
4. Never write a secret into it. Name where the secret lives instead,
   such as a sops file the new session reads with `pde-sops-peek`.
5. Reread it as a reader with no earlier context. Explain each label
   and use absolute paths.

## Split ownership

- Two sessions never edit the same file, branch or worktree at once.
- The handoff names which session owns each one, and when ownership
  moves, such as "Do not edit the design until the other session says
  its new section is done."
- If the project has a tracking page, such as a roadmap note, record
  the ownership change there too.
- After the handoff, stop changing what you handed over.
- When ownership moves later, tell the new session with
  `herdr agent prompt <name> "<message>"`, and tell the user.

## Start the session

1. Check that you run inside Herdr with `test "${HERDR_ENV:-}" = 1`.
   If it fails, run no `herdr` command, not even `--help`. Tell the
   user you are not inside Herdr. Give them the handoff path and the
   short prompt from step 5, to paste into a new session they start.
   Give them no `herdr` commands, which are too long to type on a
   phone.
2. Find your own agent kind in `.result.agent.agent` of
   `herdr agent get "$HERDR_PANE_ID"`. Start the kind the user names,
   or else your own. If that command fails, or the kind is not in the
   list `herdr agent start --help` prints, ask the user which kind to
   start.
3. Create the workspace, and read the new pane from
   `.result.root_pane.pane_id`:

       herdr workspace create --no-focus --label <label> \
         --cwd <the folder the new session works in>

   `--no-focus` keeps the user's screen where it was.
4. Start the agent. Its name must match `[a-z][a-z0-9_-]{0,31}` and
   be unique among live agents:

       herdr agent start <name> --kind <kind> --pane <pane> \
         --timeout 90000

   To choose a model, pass the agent's own model flag after `--`.
   OpenCode has no model flag, so it uses the `model` set in
   `~/.config/opencode/opencode.json`.
5. Send a short prompt that points to the file, such as "Read the
   handoff at <path> and follow it. Load the skills it names first.
   Start by <step>. Do not edit <file> yet." Keep the brief in the
   file, since a file can be reread after compaction and a long
   pasted prompt is hard to check:

       herdr agent prompt <name> "<prompt>" --wait \
         --until working --until blocked --timeout 30000

   Plain `--wait` waits until the agent finishes, so a long handoff
   runs past the timeout.
6. Read the state in `herdr agent get <name>`:
   - `working` means it started.
   - `idle` or `done` means it already finished. Read its reply with
     `herdr agent read <name> --source recent-unwrapped --lines 80`.
   - `blocked` means it waits at an approval or a question. Those
     belong to the user, so never answer them; tell the user instead.

If a step times out or stalls, read `herdr agent get` and
`herdr agent read` before trying again, since the prompt may have
arrived.

To start the session on another saved machine, add
`--machine <label>` right after `herdr` in the commands of steps 3 to
6, with the label from `herdr machine list`. First copy the handoff
file to the same path on that machine, since a remote session cannot
read a local path. The remote `--cwd` must be an absolute path or
start with `~/`. Read pane IDs from that machine's replies.

## Tell the user

Write the message by the `status-updates` skill. Give:

- the workspace label and the agent name
- the handoff file path
- the model the new session runs, and that the user can pick another
  in that session
- what the new session does first, and what it waits on
- what this session keeps. When the whole job moves, say this session
  keeps nothing and stops, then stop.

[references/example.md](references/example.md) shows a real handoff,
from the brief to the message. Read
[references/evaluation.md](references/evaluation.md) only when testing
or revising this skill.

## Final checks

- The handoff file is outside the repository, any pull request and
  the vault.
- It holds no secret.
- It explains every label, and every path in it is absolute.
- It names which session owns each file, branch and worktree.
- The new agent was seen `working`, or `idle` or `done` after
  finishing.
- The message to the user names the workspace, the agent, the
  handoff file and the model.

## Rules

- Use the `herdr` CLI, not a harness plugin, so this works in any
  agent.
- Never close the new workspace or agent. They belong to the user.
- Never answer the new agent's approval or question prompts.

## Remembering corrections

Follow every lesson in the Lessons section below. Each is a correction
the user made, so a lesson wins when it disagrees with the text above,
except that you still never answer the new agent's prompts. When the
user corrects how you write a handoff, start the new session or split
ownership, follow [skill-lessons](../skill-lessons/SKILL.md) with the
skill name `herdr-handoff`, and offer once to save the correction as a
lesson.

## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
