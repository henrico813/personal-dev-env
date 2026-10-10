# Use phone approval for a pull-request write

1. Install and pair `moshi-hook` with the phone.
2. Run a normal `pde-gh-write pr ...` command.
3. Approve or decline on the phone. If phone approval is unavailable, use the
   Herdr popup or run `pde-pr-approve REQUEST_ID` in a real terminal.

The writer waits for the shared local request state. If the phone answer arrives
after the request was declined, expired, or approved another way, it changes
nothing and never runs `gh`. A rerun starts a new phone ask when the recorded
ask process is gone; a live ask prevents a second one.
