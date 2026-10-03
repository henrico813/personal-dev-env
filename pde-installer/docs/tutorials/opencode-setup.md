# OpenCode Setup

This tutorial configures the full profile's local OpenCode attach server.

## 1. Apply the Full Profile

Run from the repository root:

```bash
pde-installer install full
```

The configuration installs the systemd units and prepares Claude Code and the
managed OpenCode Claude adapter. It does not authenticate Claude Code.

## 2. Provision Credentials

OpenCode credentials are host-local runtime state. Create
`~/.config/opencode/server.env` on the host as a regular file with mode `0600`
that defines `OPENCODE_SERVER_USERNAME` and `OPENCODE_SERVER_PASSWORD`. The
full-profile setup hook enables and starts `opencode-web.service` and
`opencode-web-health.timer` once the file exists; until then both units stay
disabled. See
[OpenCode credential lifecycle](../explanation/opencode-credential-lifecycle.md)
for the ownership boundary.

## 3. Verify the Server

```bash
systemctl --user status opencode-web.service
systemctl --user status opencode-web-health.timer
curl --fail --user opencode http://127.0.0.1:4096/global/health
```

Attach from a Git repository with the OpenCode CLI against
`http://127.0.0.1:4096`. Open `https://opencode.googungus.com` from a separate
device and authenticate. The existing edge route reaches the home listener; no
route change is needed.

## 4. Authenticate Claude Code and Discover Models

Installation prepares Claude Code and its adapter but does not authenticate
Claude Code. In a login shell, authenticate with:

```bash
claude auth login --claudeai
claude auth status --json
opencode auth login claude-code
```

Restart OpenCode after authenticating Claude Code or changing its credentials so
that the adapter can observe the updated authentication state:

```bash
systemctl --user restart opencode-web.service
```

List the Claude Code models available through the managed adapter with:

```bash
opencode models --standalone | grep '^claude-code/'
```

See [OpenCode supervision](../how-to/supervise-opencode.md) for recovery tests
and troubleshooting.
