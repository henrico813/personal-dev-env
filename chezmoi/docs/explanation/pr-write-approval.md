# Pull-request write approval

## Why this exists

GitHub pull requests cannot be deleted by anyone; they can only be closed.
A mistaken pull request is therefore permanent history, so writes need a
human checkpoint. This protects against careless mistakes, not a deliberately
hostile agent that can find another copy of `gh`.

This is not a permission rule in the agent harness, the program that runs the
agent, such as OpenCode or Claude Code, because harnesses apply different
rules, and users often set the harness to approve every command the agent asks
to run. A separate local
checkpoint keeps this decision independent of both behaviors.

Approval is a human action because agent commands must not answer their own
requests. A human can approve from Herdr, the terminal multiplexer the agents
run in; from Moshi, a phone app that the `moshi-hook` command sends yes-or-no
questions to; or with `pde-pr-approve` in a real terminal.

The request ID hashes the exact command, title, and body, including body-file
contents, plus the `--repo` value as given and the directory it was requested
from. An approval for one piece of text therefore cannot be reused for a
different request. The repository and PR title that `gh` looks up are shown
to the approver but never change the ID, so a rerun finds the same request
even when that lookup fails.

The wrapper waits 90 seconds because agent command timeouts are about two
minutes; it leaves time for approval without holding the command forever. A
detached runner keeps waiting after that, so a late approval still performs
the write, and the agent's rerun reads the saved result. Requests last four
hours.

The full-profile guard makes raw `gh pr create` fail instead of skipping
approval. Long-running agent clients may need restarting after installation;
OpenCode servers and clients also need a restart to load the plugin that
passes the session ID to requests.

## How the pieces fit

1. The pull-request skill tells the agent to call `pde-gh-write`.
2. The Go writer allowlists pull-request operations and records the exact
   command, title, and body.
3. Herdr, Moshi, or `pde-pr-approve` records the human answer.
4. The writer, or the detached runner after the writer stops waiting, consumes
   that one-time answer and invokes the real `gh` under the request lock.
5. The full-profile `gh` guard blocks common raw write paths that skip approval.

Requests and answers are local records under `~/.local/state/pde/pr-write/`.
