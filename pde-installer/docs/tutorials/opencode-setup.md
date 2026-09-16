# OpenCode Setup

This tutorial configures the full profile's local OpenCode attach server.

## 1. Apply the Full Profile

Run from the repository root:

```bash
pde-installer install full
```

The configuration installs the shell helpers and systemd units. With a regular
credential file, the setup hook enables the units, restarts the service, and
starts the health timer. Until credentials exist, both units stay disabled.

## 2. Create Credentials

Start a login shell and create a host-local password:

```bash
exec zsh -l
ocw-password
```

The helper writes `~/.config/opencode/server.env` with private permissions and
automatically enables and starts `opencode-web.service` and
`opencode-web-health.timer`.

## 3. Verify the Server

```bash
systemctl --user status opencode-web.service
systemctl --user status opencode-web-health.timer
curl --fail --user opencode http://127.0.0.1:4096/global/health
```

Attach from a Git repository with:

```bash
oca
```

Open `https://opencode.googungus.com` from a separate device and authenticate.
The existing edge route reaches the home listener; no route change is needed.

See [OpenCode supervision](../how-to/supervise-opencode.md) for recovery tests
and troubleshooting.
