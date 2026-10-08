# Phone approval through Moshi

When `moshi-hook` is installed and paired, `pde-gh-write` starts one detached
`moshi-hook ask --require-remote` for each new request. The phone receives a
short question containing the operation, repository name, pull-request target,
and title when one was supplied. The request body and command arguments do not
leave the machine through Moshi.

The installer pins moshi-hook v0.4.20. Pair the installed hook before relying
on phone approval. The popup and terminal paths remain local and work when the
hook is missing, unpaired, offline, declined, or times out.

The wrapper records a PID marker beside the request. A retry removes a marker
whose process is gone, while a live marker prevents duplicate phone asks. A
late answer is ignored when the popup or another answer has already handled
the request.
