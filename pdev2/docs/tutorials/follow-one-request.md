# Follow One Request

This tutorial sends one request through the program in a throwaway directory.
A fake `gh` prints its arguments instead of writing to GitHub, and the request
records stay in that directory. You need Go 1.21 or newer, `less`, and a real
terminal.

## 1. Build Both Commands

Run from the repository root:

```bash
REPO=$PWD
cd "$(mktemp -d)"
mkdir bin guard fake
go build -C "$REPO/pdev2" -o "$PWD/bin/pde-gh-write" .
ln -s pde-gh-write bin/pde-pr-approve
```

The program picks its behavior from the name it was started under, so the
link gives you `pde-pr-approve`.

## 2. Add Two Fake gh Commands

```bash
printf '#!/bin/sh\necho "guard: use pde-gh-write" >&2\nexit 2\n' > guard/gh
printf '#!/bin/sh\necho "fake gh: $*"\n' > fake/gh
chmod +x guard/gh fake/gh
export PATH="$PWD/bin:$PWD/guard:$PWD/fake:$PATH" XDG_STATE_HOME="$PWD/state"
```

The program treats the first `gh` on `PATH` as the guard that blocks raw
writes, and runs the next one. Here that is `fake/gh`. Setting
`XDG_STATE_HOME` keeps the records out of your real state directory.

## 3. Send a Request

```bash
pde-gh-write pr create --title "Tutorial" --body "Hello" &
```

It prints `waiting for approval; run pde-pr-approve ID in a terminal` and
waits up to 90 seconds. The record is now in `state/pde/pr-write/ID.json`
with `"state":"pending"`.

## 4. Approve It

Copy the ID from that line, then run:

```bash
pde-pr-approve ID
```

`less` shows the request. Press `q`, type `yes`, and press Enter. The waiting
writer sees the approval and runs the fake `gh`, which prints
`fake gh: pr create --title Tutorial --body Hello`. Run `wait` to collect the
background job.

## 5. Rerun the Same Command

```bash
pde-gh-write pr create --title "Tutorial" --body "Hello"; echo "exit $?"
```

It prints `pde-gh-write: already ran ID, exit 0` and does not call `gh`
again. Change the title or body and the command becomes a new request with a
new ID.

When you are done, close the terminal or delete the temporary directory.

[How a request runs gh once](../explanation/request-flow.md) explains what
happened at each step.
