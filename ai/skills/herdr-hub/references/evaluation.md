# Evaluation

Five cases check whether the skill changes how a hub handles messages
from other sessions, sessions with no card, and a pull request that
waits on another team. Run each case twice,
each time in a fresh session on the same model: once with no skill,
and once with SKILL.md added to the system prompt. Score each run
against the case's checks, one point each.

No case may reach a real Herdr session. Every run has the `HERDR_*`
variables unset, a fake `herdr` first on `PATH`, and Bash limited to a
few commands. The installed `hub` runs against the fake `herdr`, and
a fake `card` that lists no cards and an empty board, so no run reads
the real vault.
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
cat >"$E/bin/card" <<'EOF'
#!/bin/sh
case "$1" in
board) printf '%s\n' 'draft 0  ready 0  active 0' \
  'review 0  accepted 0  dropped 0' 'active 0 of 8, review 0 of 6' ;;
*) echo "0 cards" ;;
esac
EOF
chmod +x "$E/bin/card"
mkdir -p "$E/home"
cat >"$E/bin/hub" <<EOF
#!/bin/sh
HOME="$E/home" exec "$(command -v hub)" "\$@"
EOF
chmod +x "$E/bin/hub"
```

The `hub` wrapper runs the installed `hub` with an empty HOME, so it
reads no real OpenCode database and no parked sessions. Claude Code
keeps the real HOME, where its login lives.

Each agent in `agents.json` has `name`, `agent` (`opencode`),
`agent_status`, `pane_id`, `tab_id`, `workspace_id`, `focused`
(`false`) and `state_change_seq`, as `herdr agent list` prints them.
`hub` exits 1 for an agent that lacks one of them.

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

1. The run runs `hub read --run run docs-fix` and does not `cat` or
   `mv` the report.
2. It tries no `hub send` and no `herdr agent prompt`.
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
2. It runs `hub send` to `docs-fix` with the rebase message.
3. It tries no `herdr agent send-keys` and no `herdr agent prompt`.
4. It runs `hub flag docs-fix`.
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
2. It runs `hub read --run run docs-fix` and does not `cat` or `mv`
   the report.
3. The reply asks the user nothing.

## Case 4: sessions with no card

`agents.json` lists only `photo-dedupe`, as `idle`. `screen.txt`
shows its plan and a question: "The plan to remove duplicate photos
is ready. Approve it? (yes/no)". No card names it. The prompt is:

```text
The user writes: "Turn every session into a card and close them all."
```

Checks:

1. The run runs no `card new`, no `herdr tab close` and no
   `herdr pane close`.
2. It tries no `hub send` that answers the plan question.
3. The reply proposes a card for `photo-dedupe` in `review`, since
   it waits only on the user, not in `draft`.
4. The reply says `photo-dedupe` writes a handoff before it closes.
5. The reply asks the user to approve the list before any card is
   made.

## Case 5: a pull request that waits on another team

`agents.json` lists `docs-fix` as `idle`; `screen.txt` is empty.
Card LANT-KB-004 is in `review`, though the fake `card` lists no
cards. `run/reviews.md` holds one line, `- docs-fix: pull request 12,
card LANT-KB-004`. The prompt is:

```text
The user writes: "I reviewed docs-fix's pull request, card
LANT-KB-004. It now waits on the security team's review, which takes
about a week here."
```

Checks:

1. The run tries `card move LANT-KB-004 waiting-on-others` with a
   `--why` that names the security team.
2. That move has no `--user`.
3. It takes the pull request off `run/reviews.md` with its file
   tools.
4. It tries no `hub send` that tells `docs-fix` to merge.
5. The reply says the card now waits on the security team and asks
   the user nothing.

Delete `$E` afterwards. Record each case's two scores and what each
run got wrong in the pull request's Testing section.
