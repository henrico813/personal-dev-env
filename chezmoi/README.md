# Chezmoi Source

This directory owns PDE home configuration and checksummed external content.
Every `pde-installer install` applies it during reconciliation. The installer
passes `PDE_PROFILE=full|terminal` and `PDE_COLOR_PROFILE` while rendering it;
apply through the installer so component and visual selections match.

Edit files here using chezmoi source names. Add remote archives to
`.chezmoiexternal.toml.tmpl` with a SHA-256 checksum.
`.chezmoidata.json` owns shared terminal palettes and application theme names;
templates should consume those values instead of embedding theme colors.

Preview and apply configuration changes from `pde-installer/`:

```bash
go run . install --dry-run --repo-root ..
go run . install --repo-root ..
```

The installer snapshots changed targets before apply. A failed apply restores
the snapshot. Modifier scripts preserve selected user-owned settings. A JSONC
modify-template preserves parsed values but may normalize formatting and remove
comments when it emits the merged file.

## Terminal Workspaces

Use tmux as the Moshi attachment and recovery layer, then run Herdr inside its
managed window for Local and saved SSH-machine workspaces. Create a fresh tmux
session with `moshi DIR` or `tmux new-session -s NAME -c DIR`, then initialize
it with:

```bash
tw init DIR
```

Initialization reads the private, host-local `~/.config/pde/tw.yml`. It rejects
symlinks, files owned by another user, and files accessible by group or other
users. The file defines commands for the minimum managed windows:

```yaml
herdr: herdr
wallace: >-
  ssh -tt -o BatchMode=yes -o ClearAllForwardings=yes
  -o ForwardAgent=no -o ForwardX11=no wallace
  'tmux new-session -A -s workspace'
shell: ""
```

`tw init` reuses a lone initial `zsh` window as `shell`, then adds missing
`herdr`, `wallace`, and `shell` windows without changing extra windows, panes, or
running commands. Plain `tw DIR` retains its original behavior: it creates a
project window with left, top, bottom, and right panes, using positional commands
or the project's `left`, `top`, `bottom`, and `right` keys. Startup commands for
managed role windows run only when a window is first created. Plain `tw` still
reads project `.tw.yml`, so inspect it before trusting it. Host configuration
and Herdr's saved-machine catalog remain user-owned state.

Use `Ctrl-A` for the outer tmux and `Ctrl-B` for Herdr. In the Wallace wrapper,
`Ctrl-A Ctrl-A` forwards the prefix to Wallace's tmux. Herdr uses its mobile
status header and full-screen machine switcher at every terminal width. Machine
labels distinguish Local and Wallace agents. The managed config is validated
with Herdr 0.9.1, so run `herdr config check` after upgrades.

See the [maintenance guide](../pde-installer/docs/how-to/update-chezmoi-content.md).
