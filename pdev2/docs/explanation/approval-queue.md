# The Approval Queue

You may run several agents at once, and they can ask for approval within
seconds of each other. Before the queue, each request opened its own popup,
and a closed popup never came back: the request could then be answered only
on the phone or with `pde-pr-approve ID`.
This page explains how one popup works through every waiting request, how
to set one aside and find it again, and how Herdr shows which agent is
waiting. It builds on
[Approving in the Herdr popup or on the phone](popup-and-phone-approval.md),
which draws the popup.

Two terms come up throughout. The queue is every request that is still
waiting and not dismissed, oldest first. The inbox is
`pde-pr-approve --herdr-inbox`, meant to run from a Herdr key, which reopens
requests you set aside.

## The Queue: "1 of 2"

A single popup works through the queue: after you answer or dismiss one
request, the popup shows the oldest one still waiting, until none are left.
The order is the time each request was made. The first line shows where
you are, such as `1 of 2`.

Each new request still asks Herdr to open the popup for it, so you do not
have to look for it. Whichever popup you answer in moves on through the
same queue.

## Dismissing, and the Inbox

`q` or Esc dismisses a request. It keeps waiting: you can still answer on
the phone or in a terminal. But the queue skips it, so it no longer opens
on its own. Dismissing does not restart the request's four hours, so
dismissing and reopening a request cannot keep it waiting longer.

The inbox brings dismissed requests back. It opens the popup on the oldest
waiting request, dismissed or not, and then continues through the queue.
With nothing waiting, it shows a `No approvals waiting` notification
instead, so the key never seems to do nothing.

`g` closes the popup and moves you to the pane of the agent that asked, so
you can see what it is doing before you answer.

## The Sidebar Marker: "Approval waiting"

While a request waits, Herdr's sidebar shows `Approval waiting` on the pane
of the agent that asked. It goes away when the request is answered or
dismissed. Herdr also drops it after four hours, when the request expires,
so a marker is not left behind if the program never clears it.

The program finds the agent's pane through its OpenCode session:
`OPENCODE_SESSION_ID` in the agent's environment names it, and
`herdr agent list` shows which pane runs it. Without the session ID, there
is no marker and `g` only closes the popup. Herdr records only the main
OpenCode session, so the same is true for a request from a subagent.

One agent can ask twice. The marker belongs to the pane, not the request,
so it stays until no other waiting, undismissed request from that pane is
left.

If you answer before the marker appears, it is removed.

Exact keys, exit codes, and record fields are in the
[reference](../reference/requests.md#herdr-popup).
