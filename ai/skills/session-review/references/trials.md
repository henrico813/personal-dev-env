# Trials

A trial checks whether a model can do one job of a role, such as the
hub or a worker. Each case is the situation just before a mistake the
user corrected, and the correction is its answer key. The script runs
each case several times on the current rules and grades every answer
the same way, so two reviews' pass rates can be compared.

## Run the trials

    python3 ~/.agents/skills/session-review/scripts/trials.py

It runs every case in `~/.local/state/pde/session-reviews/trials/`
three times on `goog/qwen3.8` and prints one line per case:

    stale-ask 2/3 rules 4f1c2a9e0b7d - missed pass /recheck/

The line gives the passed runs, the hash of the rules files, and the
first failure's reason. The model gets no tools, since a prompt
describes real actions, such as merging, that it might try. Options:

- `--case ID` runs one case; repeat it for more.
- `-n 5` runs each case five times. Use at least 3, since one run
  says little about a model whose answers change between runs.
- `--model` and `--variant` choose the model and its effort.
- `--timeout` sets the seconds each run may take; the default is 600.

Each answer is saved in `answers/<time>/`, and each run adds a line
to `results.jsonl`, both in the trials folder. A line names the
model, the effort, the rules hash and the case file's hash. Compare
pass rates only between lines with the same rules hash, or say the
rules changed.

## Write a case

Cases quote the user's notes, so they live in the trials folder's
`cases/`, never in a repository. Find a correction in a hub run
folder's `rules.md`, `notes.md` or `decisions.md`, and save it as
`cases/<id>.json`:

```json
{
  "id": "stale-ask",
  "source": "<run folder>/notes.md:41",
  "rules": ["~/.agents/skills/herdr-hub/SKILL.md"],
  "prompt": "You are the hub. Your last check of the approvals was six minutes ago. You are about to tell the user 'Approve #509'. What do you do first?",
  "pass": ["recheck|check again"],
  "fail": ["^approve #509"]
}
```

- `id`: the name the summary line and `--case` use.
- `source`: where the correction is logged, for people.
- `rules`: the files the model reads first, in order. The script
  reads them as they are at run time.
- `prompt`: the situation before the mistake, then one question.
  Leave out the correction and any hint of it.
- `pass`: patterns that must all match the answer, ignoring case.
  Match only what the corrected behavior says, such as "recheck".
- `fail`: optional patterns that must not match. Match the logged
  mistake, such as listing the ask without a check.

Never copy a secret from a run folder's notes into a case. Describe
the situation without it.

A bad file or a missing rules file stops the script before any model
runs. Try a new case on a cloud model first: if a strong model fails
it, the prompt or the patterns are wrong, not the local model.
