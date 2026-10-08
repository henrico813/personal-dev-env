# OpenCode server refresh messages

The installer prints this message after applying managed configuration:

```text
Open new shells or tmux panes to use the updated PATH.
```

For each matching process it reports:

```text
OpenCode server PID 1234 has the old PATH; restart it with: kill -TERM 1234
```

When a terminal is available, it asks:

```text
Restart the OpenCode server now so agents use the gh guard? [y/N]
```

Answer `y` to send `SIGTERM` to the listed user-owned servers. Any other
answer, or a non-interactive install, leaves them running. The installer then
prints that OpenCode clients start the server again on demand.
