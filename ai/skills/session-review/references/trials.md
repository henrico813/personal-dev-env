# Trials

A trial checks whether a model can do one job of a role, such as the hub
(the Herdr session that runs other sessions) or a worker (a session the
hub starts for one task). Each case is the situation just before a
mistake the user corrected, and the correction is its answer key. The
script runs each case several times on the current rules and grades
every answer the same way, so two reviews' pass rates can be compared.

A skill can keep cases too, to check a change to its text. "Run a
skill's cases" says how.

## Contents

- Run the trials
- Write a case
- Run a skill's cases
- Weekly runs
- Write a skill case
- Write a correction case

## Run the trials

    python3 ~/.agents/skills/session-review/scripts/trials.py

It runs every case in `~/.local/state/pde/session-reviews/trials/`
three times on the local model by default, and prints one line per
case:

    stale-ask 2/3 rules 4f1c2a9e0b7d - missed pass /recheck/

The line gives the passed runs, the hash of the rules files, and the
first failure's reason. The model gets no tools, since a prompt
describes real actions, such as merging, that it might try. The
prompt tells it so, and an answer that writes a tool call as text
fails with "tool call instead of answer". Options:

- `--case ID` runs one case; repeat it for more.
- `-n 5` runs each case five times. Use at least 3, since one run
  says little about a model whose answers change between runs.
- `--model` and `--variant` choose the model and its effort.
- `--timeout` sets the seconds each run may take; the default is 600.

Run the same cases with `--model` set to the cheap model, as
`model-routing` picks. Compare the pass rates before you choose a
model for the job.

Each answer is saved in `answers/<time>-<random suffix>/`, and each run
adds a line to `results.jsonl`, both in the trials folder. A line names
the model, the effort, the rules hash, the case file's hash and the
OpenCode version. Compare
pass rates only between lines with the same rules hash, or say the rules
changed.

## Write a case

These cases quote the user's notes, so they live in the trials folder's
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

- `id`: the name the summary line and `--case` use, in lowercase
  letters, digits and hyphens.
- `source`: where the correction is logged, for people.
- `rules`: the files the model reads first, in order. The script
  reads them as they are at run time.
- `prompt`: the situation before the mistake, then one question.
  Leave out the correction and any hint of it.
- `pass`: one or more patterns that must all match the answer,
  ignoring case.
  Match only what the corrected behavior says, such as "recheck".
- `fail`: optional patterns that must not match. Match the logged
  mistake, such as listing the ask without a check. A fail pattern
  must not match an answer that refuses or rules out the wrong
  action. Check each one against a correct answer that names the
  action to refuse it, such as "I do not merge it now". This pattern
  skips "merge" right after a negation, but still fails "I will not
  wait; merging now". In JSON, double each backslash:

  ```text
  (?<!not )(?<!never )(?<!don't )(?<!won't )merg\w+ (it )?now
  ```

Never copy a secret from a run folder's notes into a case. Describe
the situation without it.

A bad file or a missing rules file stops the script before any model
runs. Try a new case first on the cheap model, as `model-routing`
picks. If that model fails it, check the prompt and the patterns
before you try other models.

## Run a skill's cases

A skill's cases live in its `evals/` folder, such as
`ai/skills/skill-development/evals/`. Run from the folder that holds
`ai/skills/`. This runs them on the skill's SKILL.md, on its SKILL.md
at `main`, and with no skill:

    python3 ~/.agents/skills/session-review/scripts/trials.py \
        --skill skill-development --at main --without

- `--skill NAME` runs that skill's cases instead of the trials
  folder's. Repeat it for more skills.
- `--all-skills` runs the cases of every skill that has an `evals/`
  folder.
- `--at REF` also runs each case on the skill's SKILL.md at that
  Git commit or branch, the old text.
- `--without` also runs each case with no skill. Such a run depends
  only on the case and the model, so a later call reuses the saved
  runs of the same case file, model and effort.
- `--changed BASE` runs the cases of each skill whose files differ
  from BASE, a commit or branch. Edits not yet committed count, but
  new files that git does not track do not. When no changed skill has
  cases, it says so and exits 0.
- `--gate` runs only correction cases, as "Write a correction case"
  says, and exits 1 when one fails more than half its runs on the
  skill text `new`. For each such case it prints a line at the end,
  such as `blocked: public-name-says-what new 1/3`.
- `--check-cases` checks the chosen cases and their known answers,
  and runs no model.

A merge check runs:

    python3 ~/.agents/skills/session-review/scripts/trials.py \
        --changed main --gate

A run gets only SKILL.md, since the model has no tools to open the
skill's other files. The line for each case gives the passed runs on
each skill text, `new`, `old` and `none`:

    plan-owner new 3/3 old 1/3 none 0/3 saved - old: missed pass /owner/

`saved` after a skill text marks its runs as reused. A case that
passed every run of every skill text ends with "every run passed, so no
difference". It cannot show what the skill changes, so make it harder
or drop it. Each line in `results.jsonl` also names its skill, its
skill text and the commit its SKILL.md came from, or none for an
uncommitted edit.

## Weekly runs

On a full install, a timer runs every skill's cases once a week on
the first Review model in `model-routing`, each case also once with
no skill to compare, from the checkout the installer used. It uses the
default scorer, described below. It runs whatever that checkout holds
at the time, such as a branch left checked out. Move the timer to the
local model only when `--check-scorer` gives the local scorer the same
total as the cloud one. When `model-routing` changes its Review model,
run every case once by hand with `--all-skills` rather than wait for
the timer. To see the last call that ran each skill's cases:

    python3 ~/.agents/skills/session-review/scripts/trials.py \
        --last-validated

It runs nothing and prints one line per skill:

    skill-development 2026-11-20 claude-code/claude-sonnet-5 13/15 rules 1a2b3c4d5e6f

The line gives the day, the model, and how many runs of the skill's
own text passed in that call. When the count drops and the rules hash
stays the same, the text did not change, so compare the model and the
OpenCode version in `results.jsonl`. To start the timer's job now
without waiting for it to finish:

    systemctl --user start --no-block pde-skill-evals.service

Then `journalctl --user -u pde-skill-evals.service` shows each case's
line as it ends.

## Write a skill case

A skill case has the same fields as a case above, with two changes:

- `rule` replaces `rules`. It names the SKILL.md heading the case
  tests, such as "Bed names". The `skill-development` skill's check
  script fails a case whose heading is gone.
- It describes a made-up situation, since anyone who can read the
  skill can read its cases.

Save it as `ai/skills/<skill>/evals/<id>.json`.

## Write a correction case

A correction case is a skill case made from a correction the user
gave, such as "a one-word public name says nothing". When a pull
request changes a skill, the merge checks run that skill's correction
cases three times each, with `--changed` and `--gate`. A case that
fails 2 of the 3 runs blocks the merge. One failed run does not,
since a model's answers change between runs. Other cases only report,
in the weekly run.

A correction case adds these fields to a skill case:

```json
{
  "id": "public-name-says-what",
  "source": "the user's corrections to one-word public names",
  "rule": "Function names",
  "correction": true,
  "prompt": "You review a pull request in a Go repository. ...",
  "pass": ["(?s)\\bResolve\\b.{0,400}(rename|vague|unclear)"],
  "judge": ["Does the answer say the name Resolve does not tell a caller what it returns, and suggest a name that does?"],
  "good": "pricing/pricing.go:12 `Resolve` is unclear: a caller cannot tell from the name what the function returns. Rename it to something like `PriceForPlan`, which names the price it gives and the plan it prices.",
  "bad": "pricing/pricing.go:12 `Resolve` is a vague name; rename it to `ResolveIt`, which reads more naturally and keeps the verb."
}
```

- `correction`: `true` marks a case made from a correction. Only these
  cases can block a merge.
- `judge`: optional yes-or-no questions, for what a pattern cannot
  tell, such as whether a suggested name says more than the old one.
  A blind scorer answers them only when every pattern passes. It sees
  the prompt, the answer and the questions, never the skill, the
  text's name or the model. A run passes only on a yes to each.
- `good` and `bad`: a right answer and a wrong one, the known
  answers. A correction case or a judged case needs both. The script
  stops before any run when `good` misses a pattern, or when `bad`
  passes every pattern of a case with no `judge`. In a judged case,
  make `bad` a near miss that passes the patterns, so only the scorer
  can fail it and `--check-scorer` tests it on a hard answer.
- `pass` is optional in a case with `judge`.

Write the prompt as a made-up situation that tests the rule without
naming it. Use other names than the skill text's examples, so an
answer that copies an example does not pass. Run the case once with
`--at main --without` before you commit it. A case that passes on the
old text and with no skill does not show that the fix works.

A run gets only SKILL.md. For a fix in a reference file, the case
checks that the answer says to read that file at the right step.

The scorer is `--scorer`, by default the first Review model in
`model-routing`, at medium effort. To check a scorer, run it on each
judged case's known answers:

    python3 ~/.agents/skills/session-review/scripts/trials.py \
        --all-skills --check-scorer --scorer goog/qwen3.8

It prints how many of the scorer's verdicts were right, on each
case's good answer and its bad one, then in total:

    public-name-says-what good 3/3 bad 3/3
    scorer goog/qwen3.8 agreed 6/6

Make a local model the default scorer only when its total equals the
cloud scorer's on the same cases.
