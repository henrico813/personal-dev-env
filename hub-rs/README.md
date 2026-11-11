# hub in Rust

The `hub` command, rewritten in Rust. A hub is the Herdr session that
other sessions report to, and it runs `hub` to watch, message and
tidy them. The Python hub in `../hub/` stays the one agents and skills
use until this program replaces it.

pde-installer builds this folder on the Development tier and installs
it as `~/.local/bin/hub-next`, so you can try it beside the Python
`hub`. So far it has `flag`, `unflag`, `go` and `gather`.

```bash
make build   # target/release/hub
make check   # format, lint and test, as CI does
make test
```

Exit codes: 0 done, 1 an outside system such as Herdr failed, 2 wrong
usage, 3 not done, such as when no hub is running. Each failure prints
its reason.

## Differences from hub

- `hub-next --version` prints `hub 0.2.0`, since the binary already
  calls itself `hub`. The Python hub prints `hub 0.1.0`.
- `hub-next go` and `hub-next gather` exit 3, not 2, when no hub is
  running, so a script can tell this from a mistyped command.
- `hub-next flag` on a working session marks it on idle and done, so
  "needs you" shows once it stops. The Python hub marked the working
  state, which showed "needs you" while the session still worked.
