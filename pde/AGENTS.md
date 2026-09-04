# PDE

PDE configuration is stored in `chezmoi/`; installation is owned by the
`pde-installer/` Go module. The separate `pde` application is vault-only.

## Key Files

- `pde-installer/internal/installer/`: command orchestration and host checks.
- `pde-installer/test/`: Docker-based installer tests and verification scripts.
- `chezmoi/`: home configuration plus pinned external assets.
- `cli/`: vault-only `pde` command.

## Working Rules

- Keep commands `install`, `update`, `doctor`, `list`, and `config`; only
  `install` accepts a profile.
- Reject UID 0 for mutations.
- Use apt for Ubuntu dependencies and sudo only for apt.
- Keep other installer destinations under HOME.
- Keep tmux pinned and verified; install its static release under HOME.
- Keep checkout detection compatible with cwd, `--repo-root`, and `PDE_REPO_ROOT`.
- Route mutations and external commands through `Runner`.
- Keep `pde` dedicated to vault configuration and lookup.
- Run Go tests, vet, shell syntax checks, and `./pde-installer/test/run-tests.sh smoke` after installer changes.

## Entry Points

```bash
go build -C pde-installer -o ~/.local/bin/pde-installer .
pde-installer install

go build -C cli -o ~/.local/bin/pde .
pde vault --help
```
