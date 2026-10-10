# Pull-request write approval

## Why this exists

GitHub pull requests cannot be deleted by anyone; they can only be closed.
A mistaken pull request is therefore permanent history, so writes need a
human checkpoint. This protects against careless mistakes, not a deliberately
hostile agent that can find another copy of `gh`.

This is not a harness permission rule because harnesses apply different rules,
and users rely on `--auto` to approve every harness ask. A separate local
checkpoint keeps this decision independent of both behaviors.

A real terminal is required because agent commands run without one. The agent
can start the helper, but it cannot type the approval answer.

The request ID hashes the exact command, title, and body, including body-file
contents. An approval for one piece of text therefore cannot be reused for a
different request.

The wrapper waits 90 seconds because agent command timeouts are about two
minutes; it leaves time for the popup without holding the command forever.
Requests last four hours and popups reopen on tmux attach because phone
connections can drop when the app is sent to the background.

The guard and PATH order make raw `gh pr create` fail instead of skipping
approval. Already-open shells and long-running processes keep the old PATH
until they are restarted.

## How the pieces fit

1. The pull-request skill tells the agent to call `pde-gh-write`.
2. The wrapper allowlists pull-request operations, records the exact command,
   title, and body, and opens a tmux popup when a client is available.
3. `pde-pr-approve` shows the request and accepts exactly one `y` key.
4. The wrapper consumes that one-time answer and invokes the real `gh`.
5. The `gh` guard blocks common raw write paths when an agent skips the wrapper.

Requests and answers are local files under `~/.local/state/pde/`. Tmux
reattachment can recreate a missing popup, but it does not create an approval.
