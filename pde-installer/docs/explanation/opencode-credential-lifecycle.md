# OpenCode Credential Lifecycle

OpenCode credentials are host-local runtime state, not PDE configuration.
They are provisioned on the host, consumed by the server, and never stored in
the repository.

## Bootstrap Order

The full profile applies the systemd unit. The post-apply script reloads
systemd, then enables and restarts the service only when
`~/.config/opencode/server.env` is a regular file. This avoids starting an
unsecured server during a fresh installation. Re-running `chezmoi apply` uses
the same ordered activation when the file is present.

The service reads the file through systemd's `EnvironmentFile` support and
refuses to start if either credential is missing or empty. The health probe
reads authentication through curl's standard input, keeping the password out of
curl's process arguments.

## Ownership Boundary

PDE manages the service definition. Each host owns its credential file. The file
is not part of chezmoi state and must not be copied between hosts.

## Profile Boundary

The systemd unit is a full-profile feature. Terminal installs ignore the service
unit. This keeps OpenCode credentials and server processes out of the
terminal-only profile.
