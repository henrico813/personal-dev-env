# OpenCode Credential Lifecycle

OpenCode credentials are host-local runtime state, not PDE configuration.
They are created interactively, consumed by the server and client helpers, and
never stored in the repository.

## Bootstrap Order

The full profile applies the shell helper and the systemd unit. The post-apply
script reloads systemd, then enables and restarts the service only when
`~/.config/opencode/server.env` is a regular file. This avoids starting an
unsecured server during a fresh installation.

After `ocw-password` creates the file, it reloads systemd, enables both units,
restarts the service, and starts the health timer. The service reads the file
through systemd's `EnvironmentFile` support and refuses to start if either
credential is missing or empty. The health probe reads authentication through
curl's standard input, keeping the password out of curl's process arguments.
Re-running `chezmoi apply` uses the same ordered activation when the file is
present.

## Ownership Boundary

PDE manages the helper and service definition. Each host owns its credential
file. The file is not part of chezmoi state and must not be copied between
hosts.

[The helper reference](../reference/opencode-helpers.md) documents validation,
file modes, and replacement behavior.

## Profile Boundary

The helpers and systemd unit are full-profile features. Terminal installs omit
the shell helpers and ignore the service unit. This keeps OpenCode credentials
and server processes out of the terminal-only profile.
