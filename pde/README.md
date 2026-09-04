# PDE

`pde` stores and resolves vault paths. It does not install the development
environment. See [`pde-installer`](../pde-installer/README.md) for installation.

## Build

```bash
go build -C cli -o ~/.local/bin/pde .
export PATH="$HOME/.local/bin:$PATH"
```

## Vaults

```bash
pde vault main set ~/Documents/main-vault
pde vault work set ~/Documents/work-vault
pde vault default set main
pde vault path default
pde vault locate README
```

Run `pde vault --help` for all commands. State is stored in
`~/.config/pde/config.json`.
