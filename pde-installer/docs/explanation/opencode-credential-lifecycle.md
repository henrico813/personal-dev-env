# OpenCode Credential Lifecycle

OpenCode credentials are host-local runtime state, not PDE configuration.
They are created interactively, consumed by the server and client helpers, and
never stored in the repository.

## Bootstrap Order

The full profile applies the shell helper and the systemd unit. The post-apply
script reloads systemd but does not start the service when
`~/.config/opencode/server.env` is absent. This avoids starting an unsecured
server during a fresh installation.

After `ocw-password` creates the file, the operator enables the service. The
service reads the file through systemd's `EnvironmentFile` support.

## Process Isolation

`oca` and `ocw` load credentials inside a subshell and clear inherited
credential variables before parsing the file. The caller's environment is not
modified. The parser accepts only the two expected variables and rejects
malformed input, symlinks, duplicate assignments, and empty values.

## Safe Replacement

`ocw-password` validates both prompts before touching the destination. It writes
to a temporary file in the credential directory, applies mode `0600`, and
renames the completed file into place. A cleanup trap removes a temporary file
after failure. The parent directory is enforced as mode `0700`.

The accepted password character set is intentionally limited so the generated
file remains compatible with systemd environment-file parsing.

## Profile Boundary

The helpers and systemd unit are full-profile features. Terminal installs omit
the shell helpers and ignore the service unit. This keeps OpenCode credentials
and server processes out of the terminal-only profile.
