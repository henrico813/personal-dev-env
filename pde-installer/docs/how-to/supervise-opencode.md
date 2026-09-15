# Supervise OpenCode

The full profile runs OpenCode on loopback and checks it every minute.

## Check the Units

```bash
systemctl --user status opencode-web.service
systemctl --user status opencode-web-health.timer
curl --fail --user opencode http://127.0.0.1:4096/global/health
```

`opencode-web.service` owns `opencode serve` on `127.0.0.1:4096`.
`opencode-web-health.timer` runs a curl health check every minute and restarts
only the owned service when the check fails.

## Test Attach Recovery

Stop the service, then attach from a Git repository:

```bash
systemctl --user stop opencode-web.service
oca
systemctl --user status opencode-web.service
```

For the default loopback URL, `oca` probes readiness before attaching. If the
probe fails, it requests one service restart and retries readiness up to ten
times. It reports an error without attaching when the server remains down.

## Test the Timer

Run the health check immediately instead of waiting for the next timer event:

```bash
systemctl --user stop opencode-web.service
systemctl --user start opencode-web-health.service
systemctl --user status opencode-web.service
```

## Use an Override URL

An explicit URL bypasses local readiness probing and service restart:

```bash
OPENCODE_ATTACH_URL=http://example.test:4096 oca
```

## Inspect Failures

```bash
journalctl --user -u opencode-web.service --since '-10 minutes'
journalctl --user -u opencode-web-health.service --since '-10 minutes'
```
