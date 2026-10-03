# Supervise OpenCode

The full profile runs the authenticated home OpenCode service on port 4096 and
checks `/global/health` every minute.

## Check the Units

```bash
systemctl --user status opencode-web.service
systemctl --user status opencode-web-health.timer
curl --fail --user opencode http://127.0.0.1:4096/global/health
```

`opencode-web.service` owns `opencode serve` on `0.0.0.0:4096`. It refuses to
start when either credential is missing or empty. `opencode-web-health.timer`
runs an authenticated curl health check every minute and restarts only the
owned service when the check fails. The probe supplies authentication through
curl's standard input rather than its process arguments.

## Apply a Unit Update

Use direct systemd commands for the first recovery after applying these files.

```bash
systemctl --user daemon-reload
systemctl --user restart opencode-web.service
exec zsh -l
```

## Test the Timer

Run the health check immediately instead of waiting for the next timer event:

```bash
systemctl --user stop opencode-web.service
systemctl --user start opencode-web-health.service
systemctl --user status opencode-web.service
```

## Inspect Failures

```bash
journalctl --user -u opencode-web.service --since '-10 minutes'
journalctl --user -u opencode-web-health.service --since '-10 minutes'
```
