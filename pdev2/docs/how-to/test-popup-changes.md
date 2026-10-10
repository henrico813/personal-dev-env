# Test Popup Changes Without Live Herdr

Changes to the popup, the Herdr request, or the phone ask can be tested
without opening a popup in your running Herdr or sending a question to your
phone.

## 1. Run the Tests

```bash
go test -race -count=3 ./...
```

`TestMain` in `test_setup_test.go` unsets `HERDR_ENV` and
`OPENCODE_SESSION_ID` and replaces what starts phone asks and runners, so no
test reaches a live Herdr server or phone or starts copies of the test
binary. A test that needs any of these sets up a fake itself. Keep that file as it
is; a test that reached your Herdr would open real popups while you work.

## 2. Test the Screen in a Pseudo-Terminal

The popup tests in `approval_test.go` run `askPopup` against a
pseudo-terminal from `openPTY`:

1. `setPopupSize` gives it Herdr's real 37x14 size.
2. `popupTerminal` gives the popup its own copy of the terminal descriptor.
3. `readUntil(t, master, "Typed:")` waits until the screen is drawn. Send
   keys only after this; earlier input is discarded on purpose.
4. Writing to `master` types keys. A tap is a press and release, such as
   `\x1b[<0;10;ROWM\x1b[<0;10;ROWm`, with `ROW` taken from the layout that
   `popupScreen` returns. Only the press counts, and one tap only selects a
   button, so answering takes two presses.
5. `shownBeforeAnswer` waits for text such as `tap APPROVE again to
   confirm` and fails if the popup answered first. `receive` reads the
   result and fails after 2 seconds.

Pass a settle time of 0 to `askPopup` unless the test is about the settle
pause. Queue tests use `queueTwoMerges`, which saves two pending requests and
points `approvePopup` at the pseudo-terminal.

Layout-only changes can call `popupScreen` directly with a `popupView` and a
size. Tap, Enter, and settle rules can call `popupView.handle` directly, as
`TestPopupAppliesTapAndSettleRules` does, without a terminal.

## 3. Fake the Herdr Socket

`TestHerdrPopupUsesSocketRequest` shows the pattern. It listens on a Unix
socket in a temporary directory, puts a fake `herdr` first on `PATH` that
prints `socket: PATH` for `herdr status server`, sets `HERDR_ENV=1` and a
temporary `XDG_STATE_HOME` for that test only, saves a pending record, and
calls `notifyRequest`. The listener reads the JSON request and
replies with one line, as Herdr does.

## 4. Try the Popup by Hand

From the repository root, build the program in a temporary directory and
leave one request pending. The fake `gh` answers the repository lookup
with `owner/repo` and prints everything else instead of writing to GitHub,
the fake `moshi-hook` sends nothing to your phone, and unsetting
`HERDR_ENV` keeps the request out of your running Herdr:

```bash
REPO=$PWD
cd "$(mktemp -d)" && mkdir bin guard fake
go build -C "$REPO/pdev2" -o "$PWD/bin/pde-gh-write" .
ln -s pde-gh-write bin/pde-pr-approve
printf '#!/bin/sh\nexit 2\n' > guard/gh
printf '#!/bin/sh\ncase "$1 $2" in\n"repo view") echo owner/repo ;;\n*) echo "fake gh: $*" ;;\nesac\n' > fake/gh
printf '#!/bin/sh\nexit 3\n' > fake/moshi-hook
chmod +x guard/gh fake/gh fake/moshi-hook
export PATH="$PWD/bin:$PWD/guard:$PWD/fake:$PATH" XDG_STATE_HOME="$PWD/state"
unset HERDR_ENV OPENCODE_SESSION_ID
pde-gh-write pr create --title "Try the popup" --body "Hello" &
```

Copy the ID from the line it prints, then run this in the same terminal:

```bash
stty rows 14 cols 37
PDE_REQUEST_ID=ID pde-pr-approve --herdr-popup
```

This is the command Herdr runs, at the size Herdr gives it. Your terminal
must send xterm mouse reports for taps to work; typing `yes` and Enter works
everywhere. Resize the terminal or run `reset` afterwards. The request also
started a runner, a copy of the program that keeps waiting in the
background; answering the request ends it.

Popup keys and exit codes are listed in the
[reference](../reference/requests.md#herdr-popup).
