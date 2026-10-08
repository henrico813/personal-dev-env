# OpenCode server refresh

Until the server restarts, agents running through it can still run raw
`gh pr` writes because they use the old PATH, which finds the real `gh` before
the guard. An OpenCode server is a long-running process: a new shell or tmux
pane cannot update the environment it already received.

After applying the `gh` guard, the installer scans user-owned `/proc`
processes for `opencode serve` and `opencode.exe ... serve`. It compares the
server's first `gh` on `PATH` with `~/.local/bin/gh`. A mismatch is reported so
the user can choose whether to stop the server.

The first OpenCode client starts the server automatically. Later clients and
OpenChamber share that server, so stopping it can affect all their active
sessions. Stopping sends `SIGTERM`; it does not migrate or save sessions, but
the next client starts a server with the updated PATH. Declining means those
clients keep using the old server, and its agents can still bypass the guard
until someone restarts it.
