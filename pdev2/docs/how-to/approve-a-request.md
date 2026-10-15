# Approve or Decline a Request

Use this when an agent waits for your approval to write to a pull request.
Its command prints a line that starts with
`pde-gh-write: waiting for approval`; the long ID in that line names the
request. Every popup key is in the
[reference](../reference/requests.md#herdr-popup), and the reasons for the
safeguards are in
[Why popup and phone answers are safe](../explanation/popup-and-phone-approval.md).

## In the Herdr Popup

1. Read the first lines: what is asked and of which PR, the repository,
   the Herdr workspace and agent, and the PR's title. The text that will
   be posted follows.
2. To read the whole request, type `v`. It opens in `less`; press `q` to
   come back.
3. To see what the agent is doing first, type `g`. The popup closes and
   Herdr moves to the agent's pane.
4. Tap APPROVE or DECLINE, then tap the same button again. Or type `yes`
   or `no` and press Enter.

To set a request aside without answering, press `q`. It keeps
waiting but no longer opens on its own. To bring it back, press `prefix+a`
(Ctrl+b, then a) in Herdr, or have the agent rerun the command.

A notification titled `PR approval on HOST` names the computer that
asked and, when the popup opened, its workspace. If `HOST` is another
computer, select that computer in Herdr, then go to that workspace.

Herdr shows one popup at a time. If a notification says
`Approval waiting: answer the popup in NAME`, an unanswered popup in that workspace
holds the other requests; Ctrl+b, then a, takes you there. See
[If another popup is open](debug-a-stuck-request.md#6-if-another-popup-is-open).

## When the Request Is on Another Machine

Each machine keeps and answers its own requests. With nothing waiting
here, `prefix+a` checks the other saved Herdr machines and says where
requests wait, such as `Wallace: 1 waiting in` and `notes`. A request
dismissed there counts too. To answer it, select that machine in
Herdr, in the sidebar or with the machine switcher on the phone, and
press `prefix+a` there.

`Wallace: not reachable` means the check failed or took over 20
seconds, so requests may still wait there. Check from here with
`herdr --machine Wallace pane list`.

## On the Phone

Answer the question in the Moshi app. The phone shows only the first
lines, not the text that will be posted, so use the popup or a terminal
when the text matters.

For an agent in Herdr, the question reaches the phone 30 seconds after
the request, and only if you have not answered by then. A rerun in
Herdr of a request first made outside Herdr waits the same way, though
no popup shows. A
card answered elsewhere after it arrived stays on the phone and later
shows "timed out"; answering it changes nothing.

## In a Terminal

1. Copy the request ID from the agent's `waiting for approval` line, or
   from the `id:` line that `v` shows in the popup.
2. In a terminal you type into yourself, not through an agent, run
   `pde-pr-approve ID`.
3. Read the request in `less`, then press `q`.
4. Type `yes` or `no` and press Enter.

## Decline a Request You Do Not Recognize

Requests from every agent on this computer show up in the same popup and
on the same phone, so one may not be from the agent you are watching. If
you did not expect it, decline it.

## When the Agent Reports Exit 3

Exit 3 means no one answered within 90 seconds. The request still waits:
answer it as above, and `gh` runs within a second of your approval. When
the agent runs the same command again, it prints
`pde-gh-write: already ran ID, exit N`, where `N` is the exit code of
`gh`, and what `gh` printed, such as the new PR's address.
