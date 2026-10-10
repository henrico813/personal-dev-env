# Approve a Pull-Request Write

On a full install, agents wait for you before they post to a pull request.
This page shows how to answer, and how to set up the popup and the phone
once. Why it works this way is in the
[explanation](../explanation/pr-write-approval.md).

## Answer a Request

When an agent asks, its command prints a line like this and waits up to
90 seconds:

```
pde-gh-write: waiting for approval in the Herdr popup or on the phone, or run pde-pr-approve ID in a terminal
```

Answer in one place; the first answer counts.

- **Herdr popup.** Read the summary at the top. Press `v` to read the
  full request, then `q` to return. Tap APPROVE or DECLINE, or type `yes`
  or `no` and press Enter. `q` or Esc closes the popup without answering.
- **Phone.** Moshi shows only the operation, repository, PR, and title,
  not the body. Approve or decline there when that is enough.
- **Terminal.** Run `pde-pr-approve ID` in a terminal you type in. It
  shows the full request in `less`; press `q` to close it, then type `yes`
  or `no` and press Enter. Anything else asks again. Ctrl-D quits without
  answering.

All agents on the machine share the same requests, so you may see one
from another session. Decline any request you do not recognize.

## If the Agent Stopped Waiting

If you answer after the 90 seconds, the agent's command has already
exited with code 3. Approve anyway, then ask the agent to rerun the
identical command; that rerun runs `gh`.

A rerun of a request that already finished prints its result instead of
posting again:

- `already ran ID, exit N`: `gh` ran and exited N.
- `declined ID`: you declined. This holds for four hours after the
  decline; after that, the identical command asks again.
- `interrupted ID`: `gh` started but that run died, so it may have
  posted. It is never run again; check the pull request yourself.

## Link the Popup If the Install Could Not

Each install links the popup into Herdr. If Herdr was stopped or missing
at the time, the install printed:

```
Herdr is unavailable; link later with: herdr plugin link /home/you/.local/share/pde/herdr-plugins/pde-approval
```

With Herdr running, run:

```bash
herdr plugin link ~/.local/share/pde/herdr-plugins/pde-approval
```

## Set Up the Phone

Do this once per machine. The popup and the terminal work without it.

1. In the Moshi iPhone app, open Settings, then Integrations, and copy a
   pairing token.
2. Pair this machine and install the hooks:

   ```bash
   moshi-hook pair --token TOKEN
   moshi-hook install
   ```

3. Run `moshi-hook doctor`. It reports whether pairing and the Moshi
   background service work, and what is missing.
