# Set Up or Rotate OpenCode Credentials

Use the full-profile `ocw-password` helper to create or replace the host-local
password.

## Create or Rotate the Password

```bash
exec zsh -l
ocw-password
```

The helper prompts twice without echoing input. It rejects empty, mismatched,
or unsupported values before changing the existing file.

If the service is active, the helper restarts it after the replacement. Start
an inactive service explicitly:

```bash
systemctl --user enable --now opencode-web.service
```

## Verify the Service

Confirm the file remains private:

```bash
stat -c '%a %n' ~/.config/opencode/server.env
```

The expected mode is `600`. Anonymous health requests should return `401`:

```bash
curl -sS -o /dev/null -w '%{http_code}\n' \
  http://127.0.0.1:4096/global/health
```

Use curl's password prompt to verify an authenticated request returns `200`:

```bash
curl -sS -o /dev/null -w '%{http_code}\n' \
  -u opencode \
  http://127.0.0.1:4096/global/health
```

## Recover From Invalid Input

If the prompts do not match or contain unsupported characters, the command
exits without replacing the old file. Run it again with a valid value.

If the service fails after rotation, inspect it without printing the file:

```bash
systemctl --user status opencode-web.service
journalctl --user -u opencode-web.service --since '-5 minutes'
```
