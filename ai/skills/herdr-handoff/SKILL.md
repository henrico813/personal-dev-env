---
name: herdr-handoff
description: Hands part or all of a session's work to a new agent
  session in a new Herdr tab or workspace.
  A handoff file carries everything
  the new session needs, since it starts with no earlier context. Use
  when the user asks to hand work off to a new session, to start a new
  session in a new Herdr workspace, or to move work out because it is
  independent or this session's context is full. Also use when the
  user asks to prepare long, repetitive work for a cheaper or local
  model. For a subagent whose work the session reviews, use delegate.
metadata:
  pde-workflow: "true"
---

# Herdr Handoff

Herdr is a terminal workspace manager with workspaces, tabs and panes,
and a `herdr` CLI that starts and watches coding agents. This skill
covers handing work to a new agent session in a new Herdr tab or
workspace. The new
session remembers nothing of this one, so a handoff file
gives it everything it needs.

For command syntax, read Herdr's own skill with `herdr --skill`, but
only after the check in step 1 of Start the session passes. The
installed binary is the authority, so check `herdr <group> --help`
rather than trusting the examples here.

## When to hand off

- The user asks for it.
- Part of the work is independent of yours and can run alongside it.
- Your context is above the 200k handoff level. Run `pde-context` at
  each milestone, such as a merged PR, to check.

When the user has not asked, propose the handoff and wait for a yes.
Herdr's skill says to create a workspace or tab only when the user
asks.

## Prepare the work for a cheaper model

Use this part only when the user asks to prepare long, repetitive
work for a smaller or local model. A review that repeats one check
for each of many tickets is such work. Follow
[references/cheaper-model.md](references/cheaper-model.md) before you
write the handoff file. That page moves the judgment, facts and tool
steps out of this conversation into the work's own log, so a smaller
model can run the loop from them.

## Write the handoff file

1. Save it as `~/.local/state/pde/handoffs/<date>-<slug>.md`, such as
   `20261101-design-process.md`. Get the date with `date +%Y%m%d`.
   Never put it in the repository, a pull request or the vault.
2. Use the sections and order in
   [references/handoff-template.md](references/handoff-template.md).
3. Copy into it every result that lives only in this conversation,
   such as subagent reports and review findings. The new session
   cannot see them, so they are lost otherwise. When the handoff
   pauses the work and no session takes it now, also list each
   uncommitted file, unpushed branch and worktree you leave, and each
   question waiting on the user, word for word. Leave the files as
   they are.
4. Never write a secret into it. Name where the secret lives instead,
   such as a sops file the new session reads with `pde-sops-peek`.
5. Reread it as a reader with no earlier context. Explain each label
   and use absolute paths.
6. Name each skill the work needs with the step that needs it, such
   as "Load `pull-request` before you open the PR." Name a skill to
   load at the start only when the first step needs it, since every
   loaded skill is re-read on each turn.

## Split ownership

- Two sessions never edit the same file, branch or worktree at once.
  When both need one file, split the file first, or run the sessions
  one after the other.
- The handoff lists the exact files the new session may read and the
  files it may edit, such as "Edit only docs/restore.md. Read
  src/restore.rs." A folder or a phrase such as "the docs" is not a
  list.
- The handoff names which session owns each one, and when ownership
  moves, such as "Do not edit the design until the other session says
  its new section is done."
- If the project has a tracking page, such as a roadmap note, record
  the ownership change there too.
- After the handoff, stop changing what you handed over.
- When ownership moves later, tell the user. Tell the new session,
  and the hub when one runs, with `hub send`, as Send to a session of
  [herdr-hub](../herdr-hub/SKILL.md) says. It holds the message while
  the session is busy or the user is in its pane, and a hub's
  `hub next` sends it later. Without a hub, nothing sends a held
  message, so first wait in the background with
  `herdr agent wait <name> --until idle --until done`, then run
  `hub send` with the handoff folder as its run folder. If it prints
  `held`, the user is in the pane. Delete
  `<handoff folder>/send-<name>.json` and try again later.
- The handoff tells the new session to write progress-only news, such
  as "rehearsal passed", to its own status file. It names a report
  file, `report-<name>.md` in this session's run folder or the
  handoff folder, for a question, a blocker, or finished work this
  session waits on, such as a PR opening. The new session overwrites
  the report and ends its turn. It never prompts this session.
- When this session keeps nothing and no hub runs, the handoff names
  no report file. The new session asks the user in its own pane.
- Learn of a report with one background command:

      herdr agent wait <name> --until idle --until done \
        --until blocked

  When it returns at `blocked`, tell the user. Otherwise read the
  report, then rename it to `report-<name>.read`. No report file
  means no news. Never start it on a session that is already `idle`
  or `done`, since it returns at once. After it returns at `blocked`,
  wait with only `--until idle` and `--until done`. A hub learns of
  reports with `hub next` instead, as Wake on news of herdr-hub says.
- A question in the new session's report is yours to answer. An
  approval or question shown in its own pane while it is `blocked`
  stays the user's.

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
3. Create the new session's place, and read its pane from
   `.result.root_pane.pane_id`. `--no-focus` keeps the user's screen
   where it was, so the new pane is not focused. Step 6 focuses its
   tab when the user will work in it.

   When a hub runs, make the place as Sidebar names of
   [herdr-hub](../herdr-hub/SKILL.md) says. Work with no card gets a
   tab in the hub's workspace. The hub reads its workspace from
   `$HERDR_WORKSPACE_ID`, and another session from
   `.result.agent.workspace_id` of `herdr agent get <hub>`. The hub
   keeps tab 1.

   Without a hub, when this session keeps nothing, make a tab on this
   machine in this session's workspace, read from
   `$HERDR_WORKSPACE_ID`, unless the user asks for a new workspace. A
   tab here adds no workspace to the user's sidebar.

   Give a tab and its agent the same short name, such as `docs-p2`:

       herdr tab create --no-focus --workspace <workspace> \
         --label <name> --cwd <the folder the new session works in>

   Otherwise, make a new workspace, such as for a card's worker or
   when this session keeps working beside the new one:

       herdr workspace create --no-focus --label <label> \
         --cwd <the folder the new session works in>

4. Start the agent. Its name must match `[a-z][a-z0-9_-]{0,31}` and
   be unique among live agents:

       herdr agent start <name> --kind <kind> --pane <pane> \
         --timeout 90000 -- <agent flags> > <start file>

   Make a folder with `mktemp -d` and put `<start file>` in it. Read
   the reply from that file. The file keeps the reply even if your
   tool stops waiting for the command. The agent flags are `--auto`
   and `--model`, described below. Leave out `-- <agent flags>` when
   you pass none.

   If the command stops before it writes a reply, the agent may not
   exist. Check with `herdr agent get <name>` before step 5. Start it
   again only when the agent is missing.

   For OpenCode, pass `--auto` as an agent flag, so the new session
   approves its own tool permissions instead of stopping at each one.
   Check that `.result.argv` in the reply includes `--auto`. If it
   does not, tell the user the session will stop at permission
   prompts. `--auto` still obeys rules that deny a command, and a
   pull request write still waits for the user through
   `pde-gh-write`. Give other kinds no approval flag unless the user
   names one.

   To choose a model, pass the agent's own model flag after `--`.
   For OpenCode, put `--model <provider>/<model>` first, such as
   `-- --model github-copilot/claude-sonnet-5 --auto`. Names differ
   between machines, so take one from `opencode models` on the machine
   that runs the session. PDE's `opencode` launcher makes a session on
   that model and opens it. For a name not in that list, it prints
   close names and exits, so the start fails; read them with
   `herdr pane read <pane>`. Without `--model`, OpenCode uses the
   `model` set in `~/.config/opencode/opencode.json`.
5. Send a short prompt that points to the file, such as "Read the
   handoff at <path> and follow it. Load each skill it names when it
   says. Start by <step>. Do not edit <file> yet." Keep the brief in the
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

   When the user will work in the new session, focus its tab with
   `herdr tab focus <tab ID>` once `agent get` shows any state above.
   The tab ID is `.result.agent.tab_id` in that reply.

If a step times out or stalls, read `herdr agent get` and
`herdr agent read` before trying again, since the prompt may have
arrived.

To start the session on another saved machine, add
`--machine <label>` right after `herdr` in the commands of steps 3 to
6, with the label from `herdr machine list`. First copy the handoff
file to the same path on that machine, since a remote session cannot
read a local path. The remote `--cwd` must be an absolute path or
start with `~/`. Read pane IDs from that machine's replies. A remote
session cannot be a tab in a local workspace, so make it a new
workspace on that machine. An OpenCode session there without `--auto`
may ask to access the handoff folder first. Tell the user so, since
that approval is theirs to give. When the machine's data must stay on
it, the handoff asks for counts and pass or fail results in reports,
never lists of names or file contents.

## Record the handoff on its card

When the work has a kanban card, record the handoff on the card with
the `card` command:

- When a new session takes the work, once it works, add a Log note
  with the handoff file's path, then link the new session:

      card note <KEY> "Handoff <handoff path>"
      card link <KEY> --session <name>

- When the work pauses with a handoff and no session takes it now,
  move the card from active back to ready with a note that names the
  handoff:

      card move <KEY> ready --why "Paused; handoff <handoff path>"

  A card in review cannot move back to ready. It stays in review,
  with only the note:

      card note <KEY> "Paused; handoff <handoff path>"

## Tell the user

Write the message by the `status-updates` skill. Give:

- the workspace or tab label and the agent name
- the handoff file path
- the model the new session runs, and that the user can pick another
  in that session
- what the new session does first, and what it waits on
- what this session keeps. When the whole job moves, say this session
  keeps nothing and stops. In that case, when the new session is in a
  tab of this workspace, also say the user can close this session's
  tab. Then stop.

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
- It lists the exact files the new session may read and may edit.
- An OpenCode session started with `--auto` in `.result.argv`.
- It names the new session's report file, unless this session keeps
  nothing and no hub runs.
- The new agent was seen `working`, or `idle` or `done` after
  finishing.
- When the user will work in the new session, its tab is focused.
- The message to the user names the workspace or tab, the agent, the
  handoff file and the model.
- Without a hub, when this session keeps nothing, the new session is
  in a tab of this workspace, unless the user asked for a new one or
  it runs on another machine.
- When a hub runs, the handoff says the new session sends a merge or
  pull request edit request only after the hub says the user reviewed
  that pull request.
- When the work has a card, the card has the handoff's Log note and
  the new session, or is back in ready with a note naming the handoff.

## Rules

- Use the `herdr` CLI, not a harness plugin, so this works in any
  agent.
- Never close the new session's workspace, tab or agent. When a hub
  runs, the new session closes its own tab, as Cleanup of herdr-hub
  says. Otherwise they belong to the user.
- Never answer an approval or question shown in the new agent's
  pane. Answer only what it writes in its report.
- Filter `herdr agent list` with `jq`, and never print the whole
  list. It adds thousands of tokens that every later turn re-reads.

      herdr agent list | jq -r '.result.agents[] |
        select(.name) | "\(.name) \(.agent_status)"'

## Remembering corrections

Follow every lesson in the Lessons section below. Each is a correction
the user made, so a lesson wins when it disagrees with the text above,
except that you still never answer an approval or question shown in
the new agent's pane. When the
user corrects how you write a handoff, start the new session or split
ownership, follow [skill-lessons](../skill-lessons/SKILL.md) with the
skill name `herdr-handoff`, and offer once to save the correction as a
lesson.

## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
