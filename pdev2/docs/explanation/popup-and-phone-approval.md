# Why Popup and Phone Answers Are Safe

When an agent asks to post to a pull request, two quick ways to answer
appear on their own: a small approval window, the popup, in Herdr, the
terminal program the agents run in; and a yes-or-no question on your phone
through the Moshi app. Both are easy to answer by accident: a phone tap
can land on the wrong spot, and keys typed for an agent can land in the
popup. This page explains what keeps an answer from counting unless you
meant it. The keys, sizes, and limits themselves are in the
[reference](../reference/requests.md#herdr-popup).

## The First Answer Wins

You can answer in the popup, on the phone, or with `pde-pr-approve ID` in
a terminal. Each one saves your answer only if the request is still
waiting. It checks while holding the request's lock (see
[Why gh runs only once](request-flow.md#why-gh-runs-only-once)), so two
answers cannot both be saved. A popup still showing a request answered
elsewhere stays open, and a phone question that could not be ended stays on
the phone, but a later answer from either finds the request already
answered or expired and changes nothing. So a tap on an old popup cannot
undo an approval after `gh` may have posted, and an old phone question
cannot approve a request you declined.

## Why One Tap Does Not Answer

You often answer the popup by tapping it on a phone, where a stray tap is
easy. In live use, accidental taps decided two requests.

- **The first tap only selects a button.** A second tap on the same button
  answers. Typing `yes` or `no` and pressing Enter still answers in one
  step, because a typed word is hard to enter by accident. Each request
  starts with nothing selected, so a selection never carries over to the
  next request.
- **Only the press of a tap counts.** A tap reaches the popup as two
  messages, press and release. If the release counted too, one tap would
  select a button and confirm it at once.
- **Taps and Enter do nothing for a moment after each request appears.**
  When you answer a request, the next one in the queue is drawn at once,
  in the same place, and an impatient second tap would answer it before
  you could read it. The first popup needs the pause too: Herdr opens it
  on its own and gives it the keyboard and taps while you may be typing or
  tapping elsewhere. The `one moment...` notice says why a fast `yes` and
  Enter did nothing.
- **The popup redraws as soon as its size changes,** so the buttons are
  drawn where taps are checked.

## Why Typed Keys Are Not Trusted

- **Keys typed before the popup appears are thrown away.** The program
  asks Herdr to put the keyboard on the popup when it opens, so a `yes`
  and Enter you were typing for an agent could otherwise land in the popup
  and approve.
- **A single `y` is not an answer.** An earlier version approved on one
  stray key.
- **Keys typed while `v` shows the full request are thrown away** when
  `less` closes, so a `yes` typed into its search does not answer.
- **Agent text cannot change the screen.** The agent writes the title and
  body. Characters that tell a terminal to clear the screen or move the
  cursor are shown as plain text instead, so the clear-screen code
  `ESC [2J` appears as `^[[2J`. Otherwise an agent could draw a different
  request over the real one.

## Why the Popup Shows What It Shows

You may run several agents at once, and they can ask within seconds of
each other, so each request starts with lines that say what is asked and
which agent asked. A line that cannot be found is left out rather than
guessed, so a folder is never mistaken for a repository. At Herdr's popup
size of 37 columns by 14 rows, the popup looks like this:

```
comment #12                    1 of 2  <- what is asked; queue position
octo/app                               <- repository
app / review login fix                 <- Herdr workspace / agent's title
Fix login timeout                      <- the PR's title
Looks good. One question:              <- the comment, as much as fits
should the timeout be 30s?
The old value was 10s.
Typed:                                 <- keys typed so far, then notices
v: full; g: focus; q/Esc: dismiss

             [ APPROVE ]               <- tap row 10 or 11 to approve


             [ DECLINE ]               <- tap row 13 or 14 to decline
```

The buttons are two rows tall here, which leaves six rows below the first
line for the context lines and the start of the text that will be posted.
With three-row buttons only three rows were left, which the context lines
alone filled. The typed keys and the notices share one line to save
another row; when both do not fit, the notice wins, because it says what
the next tap or Enter will do. For a `pr edit` that renames the PR and
changes its body, the text rows show the changed lines first, then the new
body:

```
-Fix login
+Fix login timeout
-Old body
+New body
New body
```

## Why One Popup Works Through a Queue

When each request opened its own popup, several agents meant several
popups, and a closed popup never came back. Now one popup works through
every waiting request, and a request you dismiss keeps waiting; the inbox
key brings it back. With nothing waiting, the inbox shows a notification,
so the key never seems to do nothing.

The `Approval waiting` marker in Herdr's sidebar is set with a four-hour
time limit, the request's lifetime, so that Herdr can drop a marker the
program never clears. The program finds the agent's pane through its
OpenCode session. Herdr does not list every session, never a subagent's,
so the program then uses the pane Herdr started the agent in. A
subagent's request marks its main agent's pane.

## Why the Popup Is Opened Through Herdr's Socket

The `herdr` command has no option that opens a popup, so the program sends
the request to Herdr's socket, a file that other programs on the same
computer use to send Herdr requests. This runs in the background, and a
failure does not stop the request: a slow or missing Herdr must not delay
it, and you can answer on the phone or in a terminal instead.

The program reads Herdr's answer, though. Herdr answers on the same
connection whether it opened the popup or refused, so a reply alone does
not mean a popup appeared. When it refused, the notification says why
instead of naming the popup's workspace.

## Why the Notification Names the Computer

Herdr can attach to other computers. It then shows their notifications
too, without saying which computer sent one, but draws a popup only
while its computer is selected. A request from the other computer
looked like a local one whose popup never appeared. So the title names
the computer that asked, and the body names the workspace the popup
opened in: Herdr opens a popup in its focused workspace, which need not
be the agent's. The popup itself does not repeat the computer's name.
You see it only with that computer selected, and the 40x16 popup has no
row to spare.

## Why One Open Popup Can Hide the Others

Herdr shows one popup at a time and refuses to open another while one is
open. An approval popup opens in the workspace in front of you; once you
switch away you no longer see it, yet it keeps every later request from
opening its own. When an approval popup was left that way, every later
request seemed to vanish, and the inbox key did nothing.

Herdr cannot say where a popup is: a popup is not a pane and has no
pane ID. So the popup saves its own workspace while it runs. When Herdr
refuses a popup, the notification names that workspace, and the inbox
key goes there, because you pressed it to see your approvals. A new
request only notifies, so an agent never moves the screen while you
work. The popup stays one 40x16 popup that works on the phone; a second
window for waiting requests would not fit there.
