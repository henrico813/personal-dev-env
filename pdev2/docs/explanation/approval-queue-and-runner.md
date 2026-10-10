# The Approval Queue and Runner

Several agent sessions can ask for approval at once, and the person answering
is often on a phone. These rules keep each answer tied to the request the
person meant, and make a late answer still count.

## Request Context

A request number alone does not say which session asked. Each record keeps
the directory it came from, the repository, the OpenCode session, and the
PR's title, and the popup and the phone question both start with the same
lines:

1. the operation and PR, such as `merge #185`;
2. the repository, or `dir: NAME` when `gh` cannot find one;
3. the Herdr workspace and session title, or the session ID;
4. the requested title, or the PR's current title when the request sets none.

The repository and PR title come from read-only `gh` lookups while the
request is created. The session ID comes from `OPENCODE_SESSION_ID`, which a
small OpenCode plugin sets for every command. The workspace, session title,
and pane come from `herdr agent list` and `herdr workspace list`. Other
lookups that fail leave their line out instead of guessing.

## The Queue

One popup works through every waiting request, oldest first, and shows its
position, such as `1 of 2`, on the first line. After each answer or dismissal
it opens the next one, so popups do not pile up on top of each other. The
order comes from the record's creation time, because the last-change time
moves whenever a record is saved again.

## The Settle Pause

For a moment after each request appears, 600 milliseconds, taps and Enter are
ignored and the popup shows `one moment...`. On a phone, an impatient second
tap would otherwise land on the next request, which has just appeared under
the finger, and approve it unread. Herdr also gives the first popup focus
while the person may be typing elsewhere.

## Confirm-Tap

In live use, accidental taps decided two requests. One tap on APPROVE or
DECLINE therefore only selects it and shows `tap APPROVE again to confirm`; a
second tap on the same button answers. Tapping the other button moves the
selection, and tapping elsewhere clears it. Only the press of a tap counts.
The release once landed on the next request's APPROVE row and approved it
too. Typed `yes` or `no` and Enter still answer in one step. Each request
starts with nothing selected.

## Dismissal and the Inbox

`q` or Esc dismisses a request: it stays pending, but the queue skips it and
it no longer opens on its own. `pde-pr-approve --herdr-inbox`, bound to
`prefix+a` in Herdr, opens the oldest pending request, dismissed or not. With
nothing waiting it shows a notification, so the key never seems to do
nothing. `g` closes the popup and focuses the requesting agent's pane.

## The Sidebar Marker

While a request waits, the requesting agent's pane shows `Approval waiting`
in the Herdr sidebar. One pane can ask twice, so the marker is cleared only
when no other pending, undismissed request from that pane remains. An answer
can arrive before the marker is set, so the notification checks the record
again after setting it and clears the marker if the request was already
answered.

## The Detached Runner

The writer stops waiting after 90 seconds, but the person may answer later.
Each new request therefore starts a runner: this program started as
`pde-gh-write --pde-runner ID` in its own session, so it outlives the agent's
command. It checks the record once a second. When the request is answered it
takes the request lock and goes through the same path as the writer, so `gh`
runs at most once whichever process gets there first. The output is saved in
the record and the agent's rerun prints it, such as a new PR's URL. On expiry
the runner exits without running anything.

A rerun of a pending request, or an approval, starts a new runner when the
saved one is gone, such as after a reboot.

The runner may be started from the popup or the phone ask, whose `PATH` may
not include `gh`. It therefore runs the `gh` path saved when the request was
created. If that path is missing, the request stays approved and rerunning
the command runs it with the `gh` found then.

To look at a request that seems stuck, see
[debug a stuck request](../how-to/debug-a-stuck-request.md).
