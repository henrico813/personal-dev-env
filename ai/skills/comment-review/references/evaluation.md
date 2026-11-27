# Evaluation

Two cases check whether the skill changes what an agent does with
a comment review. Run each case twice in a fresh session on the same
model: once with no skill, and once with SKILL.md added to the end of
the system prompt. Score each reply against the case's checks, one
point each.

Each case gives the facts as text and asks for the commands and the
message the agent would send, so no run changes a file or a review.

## How to run

Run in a folder from `mktemp -d`. `--safe-mode` leaves out the user's
CLAUDE.md and installed skills, so the baseline sees no rules.

```bash
sys='You are a coding agent helping a user. Reply with the shell
commands you would run, then the message you would send the user.
Run nothing.'
claude -p --safe-mode --model claude-sonnet-5 --tools "" \
  --no-session-persistence --system-prompt "$sys" "$(cat case1.txt)"
```

For the run with the skill, add `$(cat SKILL.md)` to the end of
`$sys`. Save each case's facts block below as `caseN.txt`.

## Case 1: a plan rewords an approved comment

```text
Your plan says: "Reword the doc comment on LoadAnswers in store.go to
say that it returns the newest answers first." The user approved that
comment in an earlier comment review, and no review waits for the
user now. glimmer is installed. Finish the task.
```

Checks:

1. Leaves the comment in store.go as it is.
2. Writes a change request file outside the repository that names
   store.go and LoadAnswers.
3. Passes that file to `glimmer make-feedback-request-list` with
   `--input`.
4. Leaves the approvals file as it is.

## Case 2: a review ready to send

```text
Your Go change adds three doc comments in ./store. glimmer
check-comments reports no failures, and glimmer
make-feedback-request-list printed a list of three requests, each
with reason new and next send_agent_suggestions. Finish the task.
```

Checks:

1. Saves `result.list` to a file outside the repository, with a
   `suggestion` in each request and nothing else changed.
2. Runs `glimmer send-agent-suggestions` on that file.
3. Tells the user the review is ready: Ctrl+b e, Ctrl+b E or
   `glimmer open <branch>`.
4. Lists no comments in the chat for the user to answer, and does
   not run `glimmer open` itself.
