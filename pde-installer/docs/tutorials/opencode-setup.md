# Set Up OpenCode Authentication

This tutorial configures the full PDE profile with a host-local authenticated
OpenCode server.

## 1. Install the Full Profile

From the `personal-dev-env` checkout:

```bash
go build -C pde-installer -o ~/.local/bin/pde-installer .
export PATH="$HOME/.local/bin:$PATH"
pde-installer install full --repo-root "$PWD"
```

The installer applies the shell helper and service unit but does not start the
service until credentials exist.

## 2. Create Credentials

Start a new login shell so the helper is available:

```bash
exec zsh -l
ocw-password
```

Enter the same value twice. Passwords may contain letters, numbers, `.`, `_`,
`:`, `@`, `%`, `+`, `,`, and `-`.

The credentials are stored at
`~/.config/opencode/server.env` with file mode `0600` and parent-directory mode
`0700`.

## 3. Start the Server

```bash
systemctl --user enable --now opencode-web.service
```

Check the service:

```bash
systemctl --user status opencode-web.service
```

## 4. Verify Authentication

Anonymous health requests must be rejected:

```bash
curl -sS -o /dev/null -w '%{http_code}\n' \
  http://127.0.0.1:4096/global/health
```

Expected result: `401`.

Use curl's password prompt for an authenticated check:

```bash
curl -sS -o /dev/null -w '%{http_code}\n' \
  -u opencode \
  http://127.0.0.1:4096/global/health
```

Expected result: `200`.

## 5. Use the Helpers

Attach to the local server from a project:

```bash
oca
```

Launch a manually managed web server instead of the systemd service:

```bash
systemctl --user disable --now opencode-web.service
ocw --hostname 0.0.0.0 --port 4096
```
