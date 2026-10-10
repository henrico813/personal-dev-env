# Approve a pull-request write

The agent prints the request ID when it asks for approval. Requests from other
agent sessions can appear too, so decline one you do not recognize.

## From Herdr or Moshi

Run the requested command through `pde-gh-write`. The Herdr popup shows a
summary, including the changed lines of a `pr edit`; type `v` for the full
request. The Moshi phone shows only the operation, repository, target, and
title, so use the popup or terminal when the body matters. The writer waits
up to 90 seconds and runs `gh` only after approval.

In the Herdr popup, tap Approve or Decline. You can also type `yes` or `no`
and press Enter. Type `v` to show the full request. Press `q` or Esc to close
the popup without answering.

## From a terminal

Run this in a real terminal:

```bash
pde-pr-approve REQUEST_ID
```

After `less` closes, type `yes` or `no` and press Enter. Anything else asks
again. Closing the prompt records nothing. An approval within 90 seconds lets
the waiting command run `gh` itself. If the command already exited 3 because
it gave up waiting, rerun the identical command after approving. Exit 3 means
waiting; `already ran <id>, exit N`, `declined <id>` (exit 4), and
`interrupted <id>` (exit 6) are final outcomes.

## Decline an unknown request

Type `no` for a request you do not recognize. Requests from other agent
sessions are visible because they share the same local state. In the Herdr
popup, tap Decline.

Requests expire after four hours. A declined or already-ran request keeps
that outcome for four hours, so an identical rerun reports it instead of
asking again. A declined or expired request must be requested again; do not
approve a different command by reusing an old answer.
