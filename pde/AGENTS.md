# PDE

PDE configuration is stored in `chezmoi/`; installation is owned by the
`pde-installer/` Go module. The separate `pde` application is vault-only.

## Key Files

- `pde-installer/internal/installer/`: command orchestration and host checks.
- `pde-installer/test/`: Docker-based installer tests and verification scripts.
- `chezmoi/`: home configuration plus pinned external assets.
- `cli/`: vault-only `pde` command.

## Working Rules

- Keep the exact commands `install`, `update`, `doctor`, `list`, and `config`.
- Keep all installer destinations under HOME and reject UID 0 for mutations.
- Keep pkgsrc source-only and unprivileged; never invoke apt or sudo.
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
