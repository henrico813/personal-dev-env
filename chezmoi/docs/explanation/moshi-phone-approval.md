# Phone approval through Moshi

`moshi-hook` is the command that sends questions to the Moshi phone app. When
it is installed and paired, meaning linked to your phone, the Go approval
binary starts one detached `moshi-hook ask --require-remote` process for each
new request. `--require-remote` means only the phone can answer; the hook never
falls back to asking in the terminal. The program that runs the agent's
commands kills the command's process group when the command ends, so the ask
runs in its own session.

Moshi's servers receive the same context lines the Herdr popup starts with: a
first line such as `Approve merge #185?` (`Approve create PR?` without a
target), then `owner/repo` (or `dir: NAME` when `gh` cannot tell), the Herdr
workspace and session title when known, and the PR title. Lines without a
value are left out. The request body and original command arguments do not
leave the machine through Moshi.

Pair the installed hook before relying on phone approval. The Herdr and
terminal paths remain local and work when the hook is missing, unpaired,
offline, or times out.

The request record stores the detached ask's PID while it is running. If that
process is gone, a rerun starts a new ask; a live PID prevents a second ask.
The ask clears the PID when it exits: after recording an answer, or when it
times out or gets no answer. When the request is answered in Herdr or a
terminal, runs, or expires, the ask's process group is stopped so no stale
question stays on the phone. A late answer after the request was handled
another way changes nothing and never runs `gh`.
