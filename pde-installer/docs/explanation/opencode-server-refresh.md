# OpenCode server refresh

Until the server restarts, agents running through it can still run raw
`gh pr` writes because they use the old PATH, which finds the real `gh` before
the guard. An OpenCode server is a long-running process: a new shell or tmux
pane cannot update the environment it already received.

After applying the `gh` guard, the installer scans user-owned `/proc`
processes for `opencode serve` and `opencode.exe ... serve`. It compares the
server's first `gh` on `PATH` with `~/.local/bin/gh`. A mismatch is reported so
the user can choose whether to stop the server.

OpenCode terminal sessions share the background server, so stopping it can
affect all active sessions. Stopping sends `SIGTERM`; it does not migrate or
save sessions, but the next terminal session starts a server with the updated
PATH. Declining means those sessions keep using the old server, and its agents
can still bypass the guard until someone restarts it.
