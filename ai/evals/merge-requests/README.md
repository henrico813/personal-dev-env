# Merge request cases

Each case here gives a hub the user's words from a time an agent
turned "deploy", or an approval of pull request text, into a merge
request. One more case, `merge-says-merge`, checks that a plain
"merge it" still sends one. Each case reads `ai/AGENTS.md` and the
`herdr-hub` and `pull-request` skills from the folder you run it in.

Run the cases from the top folder of a checkout, three times each:

```bash
T=~/.local/state/pde/merge-request-trials
mkdir -p "$T/cases"
cp ai/evals/merge-requests/*.json "$T/cases/"
python3 ~/.agents/skills/session-review/scripts/trials.py \
  --trials "$T" -n 3
```

To compare with main, run the last command again from a checkout of
main. It reads the rules from that folder and the cases from `$T`.
