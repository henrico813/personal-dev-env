# Pull-request write approval

## Why this exists

Pull requests are shared GitHub objects. A mistaken creation or update cannot
be removed by the agent after the fact, so pull-request writes need a human
checkpoint outside the agent process. This protects against careless mistakes,
not a deliberately hostile agent that can find another copy of `gh`.

The checkpoint is outside the harness and its `--auto` mode. The harness can
prepare and display an exact request, but it cannot approve its own write.
The normal `gh` login remains in use; the approval scripts do not need a
second token.

## How the pieces fit

1. The pull-request skill tells the agent to call `pde-gh-write`.
2. The wrapper allowlists pull-request operations, records the exact command,
   title, and body, and opens a tmux popup when a client is available.
3. `pde-pr-approve` shows the request and accepts exactly one `y` key.
4. The wrapper consumes that one-time answer and invokes the real `gh`.
5. The `gh` guard blocks common raw write paths when an agent skips the wrapper.

Requests and answers are local files under `~/.local/state/pde/`. Tmux
reattachment can recreate a missing popup, but it does not create an approval.
