# OpenCode server refresh

An OpenCode server is a long-running process. It keeps the `PATH` it received
when it started, even after an installer changes shell configuration. A new
shell or tmux pane therefore cannot update an already-running server.

After applying the `gh` guard, the installer scans user-owned `/proc`
processes for `opencode serve` and `opencode.exe ... serve`. It compares the
server's first `gh` on `PATH` with `~/.local/bin/gh`. A mismatch is reported so
the user can choose whether to stop the server.

Stopping the server sends `SIGTERM`; it does not migrate or save active
sessions. OpenCode clients normally start a server again on demand, and the
new process then inherits the updated `PATH`. Declining the prompt leaves the
server running and only affects that server's environment.
