# Use phone approval for a pull-request write

1. Install the managed moshi-hook v0.4.20 and pair it with the phone.
2. Run a normal `pde-gh-write pr ...` command.
3. Check the phone question includes the actual `owner/repository` name.
4. Approve or decline on the phone. If phone approval is unavailable, use the
   tmux popup or run `pde-pr-approve REQUEST_ID` in a real terminal.

The wrapper still waits only for the local popup. If the phone answer arrives
after the request was declined, expired, or approved another way, it does not
run `gh` and does not create a new approval.
