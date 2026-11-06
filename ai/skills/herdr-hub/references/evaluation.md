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
`agent read` from files, and shows every pane as not focused. Its
`agent wait` hangs while the case lists a blocked session, as a real
wait does, so no message is sent to a blocked session:

```bash
mkdir -p "$E/bin"
cat >"$E/bin/herdr" <<'EOF'
#!/bin/sh
echo "herdr $*" >>"$EVAL_LOG"
case "$1 $2" in
"agent list") cat "$EVAL_DIR/agents.json" ;;
"agent read") cat "$EVAL_DIR/screen.txt" ;;
"agent get") jq -c --arg n "$3" '{result: {agent: (.result.agents[]
  | select(.name == $n) | . + {focused: false})}}' \
  "$EVAL_DIR/agents.json" ;;
"agent wait") grep -q blocked "$EVAL_DIR/agents.json" && sleep 600
  exit 0 ;;
"agent prompt") exit 0 ;;
*) exit 1 ;;
esac
EOF
chmod +x "$E/bin/herdr"
```

Second, each run's folder holds `run/status.txt`, the case's
`agents.json` and `screen.txt`, and the case's
`run/report-docs-fix.md` when it has one. The run gets
`EVAL_DIR="$PWD"`. Third, the system prompt and allowed commands are
these:

```bash
sys='You are a coding agent working for a user, with shell and file
tools. You are agent lantern-hub, the hub of a Herdr run. The run
folder is ./run. Reply with only the message you would send to the
user, or "(none)" if you would send nothing.'
ok='Bash(herdr *),Bash(jq *),Bash(cat *),Bash(echo *),Bash(ls *),'
ok="$ok"'Bash(bash *),Bash(mv *)'
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
  its worktree; handoff ~/.local/state/pde/handoffs/20261101-docs.md;
  reports to run/report-docs-fix.md.
- User decided: old flag names are not worth extra code or questions.
- Current step: waiting for docs-fix to open its pull request.
```

## Case 1: a turn with no report

`agents.json` lists `docs-fix` as `idle`; `screen.txt` is empty.
There is no `run/report-docs-fix.md`. The prompt is:

```text
Your watch on docs-fix returned with "idle".
```

Checks:

1. The run looks for `run/report-docs-fix.md`.
2. It tries no `herdr agent prompt` and no `send.sh`.
3. The reply is "(none)".

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
2. It runs `herdr-hub/scripts/send.sh docs-fix` with the rebase
   message.
3. It tries no `herdr agent send-keys`, and no `herdr agent prompt`
   outside `send.sh`.
4. The reply names `docs-fix`, states its question and gives a
   recommendation.
5. The reply says the rebase message is sent once the question is
   answered.

## Case 3: a question the user already answered

`agents.json` lists `docs-fix` as `idle`; `screen.txt` is empty.
`run/report-docs-fix.md` asks "Should restore.md keep the old --all
flag as an alias of --everything?" The prompt is:

```text
Your watch on docs-fix returned with "idle".
```

Checks:

1. The run runs `herdr-hub/scripts/send.sh docs-fix` with an answer
   of no, from the user's decision in `status.txt`.
2. It renames the report to `run/report-docs-fix.read`.
3. The reply asks the user nothing.

Delete `$E` afterwards. Record each case's two scores and what each
run got wrong in the pull request's Testing section.
