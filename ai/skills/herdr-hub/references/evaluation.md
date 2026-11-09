# Evaluation

Three cases check whether the skill changes how a hub handles messages
from other sessions. Run each case twice, each time in a fresh session
on the same model: once with no skill, and once with SKILL.md added to
the system prompt. Score each run against the case's checks, one point
each.

No case may reach a real Herdr session. Every run has the `HERDR_*`
variables unset, a fake `herdr` first on `PATH`, and Bash limited to a
few commands. The installed `hub` runs against the fake `herdr`, and
a fake `card` that lists no cards, so no run reads the real vault.
Merges are not covered, since a merge files a real approval request.

## How to run

Set up each run as in herdr-handoff's
[evaluation](../../herdr-handoff/references/evaluation.md), with three
changes. First, the fake `herdr` answers `agent list`, `agent get` and
`agent read` from files, and shows every pane as not focused. It
accepts every prompt and label without sending anything:

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
"agent prompt"|"pane report-metadata") echo '{"result":{}}' ;;
*) exit 1 ;;
esac
EOF
chmod +x "$E/bin/herdr"
printf '#!/bin/sh\necho "0 cards"\n' >"$E/bin/card"
chmod +x "$E/bin/card"
```

Each agent in `agents.json` has `name`, `agent_status`, `pane_id` and
`state_change_seq`, as `herdr agent list` prints them.

Second, each run's folder holds `run/rules.md`, the case's
`agents.json` and `screen.txt`, and the case's
`run/report-docs-fix.md` when it has one. The run gets
`EVAL_DIR="$PWD"`. Third, the system prompt and allowed commands are
these:

```bash
sys='You are a coding agent working for a user, with shell and file
tools. You are agent lantern-hub, the hub of a Herdr run. The run
folder is ./run. Pass --timeout 5 to any hub next. Reply with only
the message you would send to the user, or "(none)" if you would send
nothing.'
ok='Bash(herdr *),Bash(hub *),Bash(jq *),Bash(cat *),Bash(echo *),'
ok="$ok"'Bash(ls *),Bash(mv *)'
```

For the run with the skill, add "Follow this skill. Its folder is
./herdr-hub." and `$(cat herdr-hub/SKILL.md)` to `$sys`. Read the
commands the run tried and its reply with the two `jq` commands in
herdr-handoff's evaluation. A command Claude Code refused still shows
there.

Every case uses this `run/rules.md`:

```text
Hub rules for Lantern, a tool that backs up a family photo library.
- docs-fix owns branch fix/docs and its worktree; handoff
  ~/.local/state/pde/handoffs/20261101-docs.md.
- User decided: old flag names are not worth extra code or questions.
```

## Case 1: status-only news

`agents.json` lists `docs-fix` as `idle`; `screen.txt` is empty.
`run/report-docs-fix.md` says "Rehearsal passed; going on to the pull
request." The prompt is:

```text
hub next printed "report docs-fix".
```

Checks:

1. The run reads `run/report-docs-fix.md`.
2. It renames the report to `run/report-docs-fix.read`.
3. It tries no `hub send` and no `herdr agent prompt`.
4. The reply is "(none)".

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
2. It runs `hub send` to `docs-fix` with the rebase message.
3. It tries no `herdr agent send-keys` and no `herdr agent prompt`.
4. It runs `hub needs-you docs-fix`.
5. The reply names `docs-fix`, states its question and gives a
   recommendation.
6. The reply says the rebase message goes out once the question is
   answered.

## Case 3: a question the user already answered

`agents.json` lists `docs-fix` as `idle`; `screen.txt` is empty.
`run/report-docs-fix.md` asks "Should restore.md keep the old --all
flag as an alias of --everything?" The prompt is:

```text
hub next printed "report docs-fix".
```

Checks:

1. The run runs `hub send` to `docs-fix` with an answer of no, from
   the user's decision in `rules.md`.
2. It renames the report to `run/report-docs-fix.read`.
3. The reply asks the user nothing.

Delete `$E` afterwards. Record each case's two scores and what each
run got wrong in the pull request's Testing section.
