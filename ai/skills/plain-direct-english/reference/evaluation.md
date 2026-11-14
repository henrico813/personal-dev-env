# Evaluation

Three cases check whether the skill changes the prose an agent
writes. Cases 1 and 2 come from corrections the user made. Run each
case twice in a fresh session on the same model, once without the
skill and once with SKILL.md added to the system prompt. Score each
reply one point for each check it passes.

## Contents

- How to run
- Case 1: a pull request paragraph full of labels
- Case 2: wording the user gave
- Case 3: a message full of filler

## How to run

Work in a folder from `mktemp -d`. `--safe-mode` keeps the user's
CLAUDE.md and installed skills out, so the run without the skill
sees no rules.

```bash
sys='You are a coding agent working for a user. Reply with only the
text you would send to the user.'
claude -p --safe-mode --model claude-sonnet-5 --tools "" \
  --no-session-persistence --system-prompt "$sys" "$(cat case1.txt)"
```

For the run with the skill, append `$(cat SKILL.md)` to `$sys`. Save
each case's block below as `caseN.txt` and each reply as
`replyN.md`. A check that says "the checker prints nothing" scores
its point only when the command below prints no line:

```bash
python3 ~/.agents/skills/plain-direct-english/scripts/check_sentences.py \
  replyN.md
```

## Case 1: a pull request paragraph full of labels

```text
Rewrite this paragraph from a pull request description for a reader
who has not followed the work:

"Implements B2 per the R3 outcome: the P4 hook now gates on the cfg
flag, so wt-bootstrap no longer races the seeder."

Facts: B2 is the second batch of changes to a photo backup tool. R3
was a review that asked for a setting. P4 is the pull request that
adds a check before each backup. The cfg flag is `backup.verify` in
config.toml. wt-bootstrap is the script that sets up a new working
copy. The seeder copies sample photos into that copy.
```

Checks:

1. B2, R3, P4, "cfg" and wt-bootstrap are each explained or replaced
   by plain words.
2. Each sentence has one subject and one main claim.
3. The rewrite keeps both facts: the check runs only when
   `backup.verify` is set, and setting up a working copy no longer
   clashes with copying the sample photos.
4. The checker prints nothing.

## Case 2: wording the user gave

```text
The user says: "Put this docstring on parse_rows: 'Rows already
imported are skipped by looking up their keys; the seen argument
holds the keys.'" The function's parameter is named seen_keys, not
seen. Write the docstring and tell the user what you changed.
```

Checks:

1. The docstring keeps the user's words, with `seen` changed to
   `seen_keys`.
2. The reply names that one change.
3. The passive "are skipped" stays, and the reply names the rule it
   breaks once at most.
4. The reply does not rewrite the rest of the sentence.

## Case 3: a message full of filler

```text
Rewrite this message to the user so it can be sent:

"Great question! I've implemented a comprehensive fix that should
probably ensure seamless resumes. The backup now skips files it
already copied. In summary, resumes work now. Let me know if you
want any changes!"

Facts: 12 of 12 tests pass. It was not run on Windows.
```

Checks:

1. No praise, filler word, hedge, summary or closing offer is left.
2. It says that 12 of 12 tests pass and that the fix was not run
   on Windows.
3. It says that resuming skips files already copied.
4. The checker prints nothing.
