# OpenCode Helper Reference

These helpers are available only in the PDE full profile.

## `ocw-password`

Prompts twice without echoing input and writes:

```text
~/.config/opencode/server.env
```

The file contains the OpenCode Basic-auth variables:

```text
OPENCODE_SERVER_USERNAME=opencode
OPENCODE_SERVER_PASSWORD=<password>
```

The helper requires a non-empty password made from letters, numbers, `.`, `_`,
`:`, `@`, `%`, `+`, `,`, and `-`. It creates the parent directory with mode
`0700` and the file with mode `0600`.

It rejects symlinked or non-regular destinations and replaces the file through
a same-directory temporary file. A failed validation or write preserves the
previous credential file.

## `oca`

Attaches to the local server using the repository root as the OpenCode
directory, with the current directory as fallback:

```bash
oca [opencode attach options]
```

When `server.env` exists, credentials are loaded only inside the helper's
subprocess. Without the file, `oca` preserves its unauthenticated attach
behavior.

## `ocw`

Starts the OpenCode web command with host-local credentials:

```bash
ocw [opencode web options]
```

The caller chooses the hostname and port. `ocw` does not create or manage a
systemd service.

## Service

The managed local service is:

```text
opencode-web.service
```

It reads `~/.config/opencode/server.env` and listens on `127.0.0.1:4096`.
