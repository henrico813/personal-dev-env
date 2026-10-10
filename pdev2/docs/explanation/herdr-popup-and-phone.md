# The Herdr Popup and the Phone Ask

Typing `pde-pr-approve ID` means switching to a terminal. When a request is
created, the program also opens an approval popup in Herdr and starts a
question on the phone. Whichever answer arrives first is recorded; the others
find the request already answered and change nothing.

## Opening the Popup

The popup opens only when `HERDR_ENV=1`, which Herdr sets in its panes.
`pde-gh-write` asks `herdr status server` for the server's socket and sends
one `plugin.pane.open` request on it. The Herdr command line cannot ask for a
popup, so the socket is the only way to choose that placement. The request
names the `pde.approval` plugin and its `approval` pane from the plugin
manifest that chezmoi deploys, and passes the request ID in
`PDE_REQUEST_ID`. Herdr then starts `~/.local/bin/pde-pr-approve
--herdr-popup` in a 40x16 popup, which leaves 37x14 for the screen inside
the border.

This runs in a goroutine and every error is ignored. A slow or missing Herdr
must not delay the request, and the writer may exit before it finishes.

## The Popup Screen

The popup shows a summary rather than the stored request text: lines that
say what is asked and where it came from, the changed lines of a `pr edit`,
and the start of the body. [The approval queue and
runner](approval-queue-and-runner.md) describes those lines, the queue, and
the tap rules. Agent text is shown with control characters made visible, so
it cannot move the cursor or clear the screen. Lines are cut by character,
not byte, so a multi-byte character is never split.

A tap is an xterm mouse report on standard input. Input typed before the
popup draws, or while `less` shows the full text, is discarded so that keys
meant for something else cannot answer. A lone `y` is not an answer.

## The Phone Ask

The phone ask is this same program started as `pde-gh-write --moshi-ask ID`.
It builds its question from the saved record, using the same lines as the
popup, runs `moshi-hook ask` with a four-hour timeout, matching request
expiry, and records the answer.

The agent's tool runner kills the command's whole process group when the
command ends, and that would end the ask when the writer exits, at most 90
seconds later. The ask is therefore started in its own session and never
waited for. Its process ID is saved in the record. A rerun of a pending
request starts a new ask only when that process is gone, so a live ask is
never doubled and a dead one is replaced.

When `moshi-hook` returns, the ask takes the request lock and records the
answer only if the request is still pending. An answer that arrives after the
terminal or popup answered, or after the request expired, changes nothing.
The ask never runs `gh`; the waiting writer, the runner, or a rerun does
that. An answer from the terminal or popup stops the ask, so a question that
can no longer decide anything does not stay on the phone for hours; an ask
that records its own answer simply exits. On every exit while the request is
live, the ask clears its saved process ID. A stale
ID could later belong to an unrelated process, look alive, and stop reruns
from replacing the ask.

Testing these pieces without a live Herdr or phone is covered in
[test popup changes](../how-to/test-popup-changes.md).
