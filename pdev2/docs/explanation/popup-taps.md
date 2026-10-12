# Why One Tap Does Not Answer

You often answer the Herdr popup by tapping it on a phone, where a stray
tap is easy. In live use, accidental taps decided two requests. This page
explains what the popup does so that a tap only answers when you mean it.
It builds on
[Approving in the Herdr popup or on the phone](popup-and-phone-approval.md),
which draws the popup.

## The Pause After a Request Appears: "one moment..."

For 600 milliseconds after each request appears, `one moment...` appears
after `Typed:` on the same line, and taps and Enter do nothing. Typed
letters still appear after `Typed:`, and the notice stays until the
pause ends, so a fast `yes` and Enter that does nothing still shows why.

When you answer a request, the next one in the queue is drawn at once, in
the same place. An impatient second tap would otherwise land on the new
request and answer it before you could read it. The first popup needs the
pause too: Herdr opens it on its own and gives it the keyboard and taps,
while you may be typing or tapping elsewhere.

## Tapping Twice to Confirm

The first tap on a button only selects it: the button is drawn as
`[>> APPROVE <<]` and `Typed:` is followed on its line by
`tap APPROVE again to confirm`. A second tap on the selected button
answers. A tap on the other button selects that one instead, and a tap
anywhere else, or a typed key, clears the selection. Typing `yes` or `no`
and pressing Enter still answers in one step, because a typed word is hard
to enter by accident.

Each request starts with nothing selected, so a selection never carries
over to the next request.

Only the press of a tap counts. A tap reaches the popup as two messages,
press and release. If the release counted too, one tap would select a
button and confirm it at once.

## Room for the Text That Will Be Posted

At Herdr's 37x14, the buttons are two rows tall, which leaves six rows
below the first line for the context lines and the start of the text
that will be posted. With three-row buttons only three rows were left,
which the context lines alone filled, so a comment's body or an edit's
changed lines were not shown. The typed keys and the notices, such as
`one moment...`, share one line to save another row; when both do not
fit, the notice wins, because it says what the next tap or Enter will
do.

From 16 rows the buttons are three rows tall again, and below 11 rows one
row tall.

## Redrawing When the Size Changes

The popup spends its time waiting for a tap. When Herdr resizes it, the
popup draws itself again at once, so the buttons are drawn where taps are
checked. Earlier, it redrew only after the next key or tap, and checked
that tap against the old button rows.

Exact inputs, sizes, and limits are in the
[reference](../reference/requests.md#herdr-popup).
