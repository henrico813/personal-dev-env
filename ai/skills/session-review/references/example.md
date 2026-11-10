# Example review

A review of a made-up session that planned and reviewed one pull
request for Lantern, a tool that backs up a photo library. The file
goes to `~/.local/state/pde/session-reviews/20261104-lantern-p3.md`.

## Contents

- The review file
- The message to the user

## The review file

````markdown
# Session review: Lantern P3 plan and review

## Numbers

```text
session ses_0a1b2c Lantern P3 plan and review
model claude-code/claude-opus-5-5[1m]
minutes open 64
tokens read 9,412,880 (cache read 9,201,004, cache write 208,450, uncached 3,426)
tokens written 51,230 (output 38,904, reasoning 12,326)
user messages 7, model steps 96, steps logged with zero tokens 5
peak context about 162,310 tokens at seq 801
compactions 0
skills loaded roadmap-run (seq 9), create-plan (seq 9), review-plan (seq 9), pull-request (seq 640)
subagents 4
  claude-code/claude-opus-5-5[1m] count 2 read 4,216,557 written 25,890
  openai/gpt-6-luna count 1 read 88,120 written 1,402
  goog/qwen3.8 count 1 read 31,904 written 880
largest subagents by tokens read
  ses_0a1b3d claude-code/claude-opus-5-5[1m] read 2,310,442 written 14,020 P3 planner
  ses_0a1b4e claude-code/claude-opus-5-5[1m] read 1,906,115 written 11,870 P3 fresh review
  ses_0a1b5f openai/gpt-6-luna read 88,120 written 1,402 P3 PR text cold read
  ses_0a1b60 goog/qwen3.8 read 31,904 written 880 Run P3 tests
repeated commands 2
  4x cd ~/src/lantern && uv run pytest -q
  3x planner check "/vault/LANT-003 Resume copies.md" --repo ~/src/lantern
```

## Proposals

1. Have `goog/qwen3.8` run the planner's tests by a script that
   writes only the failing tests' names. It lives in a roadmap-run
   pull request. Saves about 1.1M tokens (estimate): each of the 4
   pytest runs printed about 3,500 tokens, and about 80 later steps
   read each one again.
2. Read the plan with narrow `rg` instead of whole. It lives in a
   create-plan lesson. Saves about 0.9M tokens (estimate): the plan
   is about 12,000 tokens, it was read in full early on, and about 75
   later steps read it again.
3. Run `planner check` once after the last patch instead of after
   each. It lives in a memory. Saves about 0.3M tokens (estimate).

## Lessons

4. "Keep the key out of the PR title." -> pull-request. Passes: it
   applies in any repository.

## Fallback map

- Planner, `ses_0a1b3d`: `goog/qwen3.8` would run it. Breaks: the
  plan and code it read pass the model's 128K window. Makes it work:
  a brief for one plan step at a time. Evidence: none; no trial has
  run.
- Plan reviewer, `ses_0a1b4e`: `goog/qwen3.8`. Breaks: judging the
  plan. Makes it work: a checklist whose answers cite the lines they
  checked. Evidence: none.
- PR text cold read, `ses_0a1b5f`: runs on `openai/gpt-6-luna`, a
  cheap cloud model, so it stops when the cloud does. `goog/qwen3.8`
  can run the same brief. Evidence: the trial below.
- Test runner, `ses_0a1b60`: already on `goog/qwen3.8`, and it works.

## Efficiency map

- Test runs: 4 pytest runs printed about 14,000 tokens (estimate).
  A local model runs them by a script; the check is that the
  script writes pytest's exit code and the failing names. Saves
  about 1.1M tokens (estimate, proposal 1).
- Worktree cleanup: about 2,000 tokens (estimate). Its script
  prints `done`, so `goog/qwen3.8` can run it, briefed as
  roadmap-run's Local model section says. The saving is small.

## Trials

Reran the PR text cold read trial from the Lantern P2 review on
`goog/qwen3.8`. It found the same two unclear sentences as
`openai/gpt-6-luna`, in 9 seconds.

## Fresh check

A fresh subagent reran the script with `--session ses_0a1b2c`. It
confirmed every number and dropped no proposal.
````

## The message to the user

```text
Session review of Lantern P3 plan and review, saved to
~/.local/state/pde/session-reviews/20261104-lantern-p3.md.

9.4M tokens read, peak context about 162k, 96 model steps.

Proposals
1. Tests on qwen3.8, failing names only (roadmap-run PR). Saves
   about 1.1M tokens (estimate).
2. Read plans with narrow rg (create-plan lesson). Saves about 0.9M
   tokens (estimate).
3. One planner check after the last patch (memory). Saves about 0.3M
   tokens (estimate).

Lessons to save, pick by number
4. Keep the key out of the PR title -> pull-request

Fallback: the planner is least ready; a brief per plan step would
fit qwen3.8's window.
Efficiency: test runs on qwen3.8, about 1.1M tokens (estimate).
Trials: PR text cold read on qwen3.8 matched luna, 9 s.

Reply with the numbers to apply.
```
