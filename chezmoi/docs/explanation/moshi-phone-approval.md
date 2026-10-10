# Phone approval through Moshi

`moshi-hook` is the command that sends questions to the Moshi phone app. When
it is installed and paired, meaning linked to your phone, the Go approval
binary starts one detached `moshi-hook ask --require-remote` process for each
new request. `--require-remote` means only the phone can answer; the hook never
falls back to asking in the terminal. The program that runs the agent's
commands kills the command's process group when the command ends, so the ask
runs in its own session.

Moshi's servers receive exactly this question:
`Approve gh pr OPERATION in REPO (TARGET), titled TITLE?` The `, titled TITLE`
part is omitted when there is no title. REPO is the `--repo` value when given;
otherwise it is `current repository`. The operation, repository, target, and
optional title are sent; the request body and original command arguments do not
leave the machine through Moshi.

Pair the installed hook before relying on phone approval. The Herdr and
terminal paths remain local and work when the hook is missing, unpaired,
offline, or times out.

The request record stores the detached ask's PID while it is running. If that
process is gone, a rerun starts a new ask; a live PID prevents a second ask.
The ask clears the PID when it exits: after recording an answer, or when it
times out or gets no answer. A late answer after the
request was handled another way changes nothing and never runs `gh`.
