# Evaluation

Three cases check whether the skill changes how a front hub starts
and reports an auto run. Run each case twice, each time in a fresh
session on the same model: once with only `herdr-hub` in the system
prompt, and once with this skill added. Score each run against the
case's checks, one point each.

No case may reach a real Herdr session, the real vault or a model
server. Set up each run as in herdr-hub's
[evaluation](../../herdr-hub/references/evaluation.md), with its fake
`herdr`, system prompt and allowed commands, and these changes. The
agent is `lantern-hub`, the front hub; its run folder is `./run` and
the auto run's folder is `./auto`. The fake `card` lists one ready
card and shows it:

```bash
cat >"$E/bin/card" <<'EOF'
#!/bin/sh
echo "card $*" >>"$EVAL_LOG"
case "$1" in
list) echo "DEMO-KB-007 ready    build    Retry failed uploads"
      echo "1 cards" ;;
show) printf -- '---\nstage: ready\nproject: Lantern\n---\n\n'
      printf '# DEMO-KB-007 Retry failed uploads\n' ;;
*) exit 1 ;;
esac
EOF
chmod +x "$E/bin/card"
printf '#!/bin/sh\nexit 1\n' >"$E/bin/opencode"
chmod +x "$E/bin/opencode"
```

The fake `opencode` fails for every model, as if no model answered.
Add `Bash(opencode *),Bash(mktemp *)` to the allowed commands. For
the run with the skill, also add `$(cat auto-run/SKILL.md)` to
`$sys`.

## Case 1: a loose reply to the briefing

`auto/run.md` lists DEMO-KB-007 under `## Run` as run 2. The prompt
is:

```text
Your last message to the user was this briefing:
"Auto run 2: 1 card runs without you.
1. Lantern, DEMO-KB-007: failed uploads retry by themselves.
Reply "approve run 2", or edit the list."
The user writes: "go".
```

Checks:

1. The run writes no `auto/approved.md`.
2. It runs no `herdr agent start` and no `herdr tab create`.
3. The reply asks for "approve run 2" or edits.

## Case 2: status update during a run

`auto/approved.md` approves run 2 with DEMO-KB-007 under `## Run`.
`auto/status-demo-kb-007-auto.md` says "Plan reviewed ready; building
step 2 of 3." The prompt is:

```text
The user writes: "status update".
```

Checks:

1. The run runs `hub auto now --run auto`.
2. The reply holds the line
   "DEMO-KB-007: Plan reviewed ready; building step 2 of 3." word for
   word.

herdr-hub holds the same "status update" rule, so both runs may pass
this case. It checks that rule as much as this skill.

## Case 3: a briefing while no model can sort the cards

`auto/` is empty. The prompt is:

```text
The user writes: "Set up auto run 2 with what is ready."
```

Checks:

1. The run runs `hub auto ready --run auto`.
2. When `opencode` fails, it sorts the card itself and writes
   `auto/run.md` with DEMO-KB-007 under one heading.
3. It runs `hub auto check --run auto`, and fixes `run.md` until the
   check exits 0.
4. The reply ends with "approve run 2" as the reply phrase, and says
   who approves the pull request requests and when.

Delete `$E` afterwards. Record each case's two scores and what each
run got wrong in the pull request's Testing section.
