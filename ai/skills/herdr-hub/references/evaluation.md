# Evaluation

Three cases check whether the skill changes how a hub handles messages
from other sessions. Run each case twice, each time in a fresh session
on the same model: once with no skill, and once with SKILL.md added to
the system prompt. Score each run against the case's checks, one point
each.

No case may reach a real Herdr session. Every run has the `HERDR_*`
variables unset, a fake `herdr` first on `PATH`, and Bash limited to a
few commands. Merges are not covered, since a merge files a real
approval request.

## How to run

Set up each run as in herdr-handoff's
[evaluation](../../herdr-handoff/references/evaluation.md), with three
changes. First, the fake `herdr` answers `agent list` and
`agent read` from files. Its `agent wait` hangs, as a real wait on a
blocked session does, so no message is sent to a blocked session:

```bash
mkdir -p "$E/bin"
cat >"$E/bin/herdr" <<'EOF'
#!/bin/sh
echo "herdr $*" >>"$EVAL_LOG"
case "$1 $2" in
"agent list") cat "$EVAL_DIR/agents.json" ;;
"agent read") cat "$EVAL_DIR/screen.txt" ;;
"agent wait") sleep 600 ;;
"agent prompt") exit 0 ;;
*) exit 1 ;;
esac
EOF
chmod +x "$E/bin/herdr"
```

Second, each run's folder holds `run/status.txt` and the case's
`agents.json` and `screen.txt`, and the run gets
`EVAL_DIR="$PWD"`. Third, the system prompt and allowed commands are
these:

```bash
sys='You are a coding agent working for a user, with shell and file
tools. You are agent lantern-hub, the hub of a Herdr run. The run
folder is ./run. Reply with only the message you would send to the
user, or "(none)" if you would send nothing.'
ok='Bash(herdr *),Bash(jq *),Bash(cat *),Bash(echo *),Bash(ls *)'
```

For the run with the skill, add "Follow this skill. Its folder is
./herdr-hub." and `$(cat herdr-hub/SKILL.md)` to `$sys`. Read the
commands the run tried and its reply with the two `jq` commands in
herdr-handoff's evaluation. A command Claude Code refused still shows
there.

Every case uses this `run/status.txt`:

```text
Hub state for Lantern, a tool that backs up a family photo library.
- Sessions: docs-fix (workspace docs-fix) owns branch fix/docs and
  its worktree; handoff ~/.local/state/pde/handoffs/20261101-docs.md.
- User decided: old flag names are not worth extra code or questions.
- Current step: waiting for docs-fix to open its pull request.
```

## Case 1: status-only news

`agents.json` lists `docs-fix` as `working`; `screen.txt` is empty.
The prompt is:

```text
Message from docs-fix: "Docs tests pass. Still fixing the tutorial."
```

Checks:

1. The run records the news in `run/status.txt`.
2. The reply is "(none)".

## Case 2: a blocked session

`agents.json` lists `docs-fix` as `blocked`. `screen.txt` shows a
question: "Rename docs/reference/config.md to settings.md? (yes/no)".
The prompt is:

```text
The user writes: "Tell docs-fix to rebase on main before it opens the
pull request."
```

Checks:

1. The run never runs `herdr agent list` without a `jq` filter.
2. It tries `herdr agent wait docs-fix` with `--until idle`,
   `--until done` and `--until working`.
3. It tries no `herdr agent send-keys`, and no `herdr agent prompt`
   outside the wait command.
4. The reply names `docs-fix`, states its question and gives a
   recommendation.
5. The reply says the rebase message is sent once the question is
   answered.

## Case 3: a question the user already answered

`agents.json` lists `docs-fix` as `idle`; `screen.txt` is empty. The
prompt is:

```text
Message from docs-fix: "Should restore.md keep the old --all flag as
an alias of --everything?"
```

Checks:

1. The run tries `herdr agent prompt docs-fix` with an answer of no,
   from the user's decision in `status.txt`.
2. The reply asks the user nothing.

Delete `$E` afterwards. Record each case's two scores and what each
run got wrong in the pull request's Testing section.
