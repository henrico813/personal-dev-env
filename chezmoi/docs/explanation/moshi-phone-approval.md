# Phone approval through Moshi

When `moshi-hook` is installed and paired, `pde-gh-write` starts one detached
`moshi-hook ask --require-remote` for each new request. `setsid -f` is needed
because the agent harness kills the command's process group when the command
ends; without it, earlier phone asks were killed with the command.

Moshi's servers receive exactly this question:
`Approve gh pr OPERATION in OWNER/REPOSITORY (TARGET), titled TITLE?` The
`, titled TITLE` part is omitted when there is no title. The operation,
repository, target, and optional title are sent; the request body and original
command arguments do not leave the machine through Moshi.

The installer pins moshi-hook v0.4.20. Pair the installed hook before relying
on phone approval. The popup and terminal paths remain local and work when the
hook is missing, unpaired, offline, declined, or times out.

The wrapper records a PID marker beside the request so there is one ask per
request. A retry removes a marker whose process is gone, while a live marker
prevents duplicate phone asks. A late answer is ignored when the popup or
another answer has already handled the request.
