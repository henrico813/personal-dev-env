# Rotate OpenCode Credentials

Use the full-profile `ocw-password` helper to replace the host-local password.

## Rotate the Password

```bash
exec zsh -l
ocw-password
```

The helper prompts twice without echoing input. It rejects empty, mismatched,
or unsupported values before changing the existing file.

If the service is active, the helper restarts it after the replacement. If it
is inactive, start it explicitly:

```bash
systemctl --user enable --now opencode-web.service
```

## Verify the Replacement

Confirm the file remains private:

```bash
stat -c '%a %n' ~/.config/opencode/server.env
```

The expected mode is `600`. Verify anonymous and authenticated requests as
described in [the setup tutorial](../tutorials/opencode-setup.md#4-verify-authentication).

## Recover From Invalid Input

If the prompts do not match or contain unsupported characters, the command
exits without replacing the old file. Run it again with a valid value.

If the service fails after rotation, inspect it without printing the file:

```bash
systemctl --user status opencode-web.service
journalctl --user -u opencode-web.service --since '-5 minutes'
```
