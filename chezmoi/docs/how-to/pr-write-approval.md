# Approve a pull-request write

The Herdr popup and the Moshi phone question show each request's context
rather than its ID: the operation and PR, the repository, the Herdr
workspace and session, and the PR title. Requests from other agent
sessions can appear too, so decline one you do not recognize.

## From Herdr or Moshi

Run the requested command through `pde-gh-write`. The Herdr popup shows a
summary, including the changed lines of a `pr edit`; type `v` for the full
request. The Moshi phone shows only the operation and PR, the repository, the
Herdr workspace and session, and the PR title, so use the popup or terminal
when the body matters. The writer waits up to 90 seconds in the foreground,
and a detached runner keeps waiting after that. It runs `gh` only after
approval, even if the answer comes after the foreground command exits 3.

In the Herdr popup, tap Approve or Decline, then tap the same button again to
confirm; the first tap only selects it. Tapping elsewhere clears the
selection. You can also type `yes` or `no` and press Enter. Taps and Enter
are ignored for a moment after each request appears, so input meant for
something else cannot answer it. Type `v` to show the full request or `g` to
focus the requesting agent. Press `q` or Esc to dismiss the request without
answering; it stays pending but stops opening on its own. When several
requests wait, the first line shows the position, such as `1 of 2`, and the
popup moves to the next request after each answer or dismissal.

## Open the inbox

Press `prefix+a` (Ctrl+b, then a) in Herdr to reopen pending approvals you
dismissed. It opens the oldest pending request in the popup, or shows a
notification when none are waiting.

## From a terminal

Run this in a real terminal:

```bash
pde-pr-approve REQUEST_ID
```

After `less` closes, type `yes` or `no` and press Enter. Anything else asks
again. Closing the prompt records nothing. An answer within 90 seconds lets
the waiting command finish with the result. If the command already exited 3
because it gave up waiting, the runner performs the write after approval;
rerun the identical command to read the result. Exit 3 means waiting;
`already ran <id>, exit N` followed by the captured `gh` output,
`declined <id>` (exit 4), and `interrupted <id>` (exit 6) are final outcomes.

## Decline an unknown request

Type `no` for a request you do not recognize. Requests from other agent
sessions are visible because they share the same local state. In the Herdr
popup, tap Decline twice.

Requests expire after four hours. A declined or already-ran request keeps
that outcome for four hours, so an identical rerun reports it instead of
asking again. A declined or expired request must be requested again; do not
approve a different command by reusing an old answer.

## After deploying the session plugin

OpenCode loads plugins at startup, so restart existing OpenCode servers and
clients before expecting the session to appear in requests. Herdr picks up the
`prefix+a` key binding after `herdr server reload-config` or a restart.
