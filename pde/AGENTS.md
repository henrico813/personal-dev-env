# PDE

PDE is a bash-based installer rooted in `pde/`. It is no longer an Ansible project, so keep this guide aligned with the shell entrypoints and module layout that actually ship in the repo.

## Key Files

- `pde/pde`: main installer entrypoint for `minimal` and `full`.
- `pde/bootstrap.sh`: bootstrap entrypoint for remote installs and upgrades.
- `pde/lib/core.sh`: shared helpers for logging, linking, and package installs.
- `pde/lib/shell.sh`, `tools.sh`, `editor.sh`, `ai.sh`, `fonts.sh`: profile-specific install modules.
- `pde/config/`: tracked config files that get symlinked into the user home directory.
- `pde/test/`: Docker-based verification for installer changes.

## Working Rules

- Keep both user-facing profiles working: `minimal` and `full`.
- Keep `bootstrap.sh` aligned with the public GitHub URL and the combined repo layout.
- Config symlinks must work both when PDE is run from a repo checkout and when it is bootstrapped into `~/.pde`.
- When changing install behavior, prefer updating the relevant `pde/lib/*.sh` module instead of growing `pde/pde`.
- Run `./test/run-tests.sh minimal` after installer changes. Run the broader test set when touching profile or bootstrap behavior.

## Entry Points

```bash
./pde/pde minimal
./pde/pde full
curl -fsSL https://raw.githubusercontent.com/henrico813/personal-dev-env/main/pde/bootstrap.sh | bash -s -- minimal
```
