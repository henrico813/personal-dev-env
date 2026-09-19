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

Use tmux as the attachment and recovery layer, with separate presentation
sessions so a phone cannot resize the desktop. Start or resume the environment
for the current directory with one command:

```bash
tm
```

`tm` creates `hub` and `pocket` sessions containing Herdr and an empty shell,
plus a `dash` session containing the four-pane `tw` layout. It attaches `pocket`
at 72 columns and below and `hub` at larger widths. Use `tm hub`, `tm pocket`,
or `tm dash` to select a view explicitly. All three sessions record the project
root and use names derived from its absolute path, so environments for multiple
directories can coexist. Hub and Pocket both attach Herdr's `default` session so
they expose the same agents.

Plain `tw DIR` remains available for adding another project window with left,
top, bottom, and right panes, using positional commands or the project's
`left`, `top`, `bottom`, and `right` keys. It still reads project `.tw.yml`, so
inspect that file before trusting it. Herdr's saved-machine catalog remains
user-owned state.

Use `Ctrl-A` for tmux and `Ctrl-B` for Herdr. Herdr uses its mobile status header
and full-screen machine switcher at 72 columns and below. Machine labels
distinguish Local and Wallace agents. The managed config is validated with Herdr
0.9.1, so run `herdr config check` after upgrades.

See the [maintenance guide](../pde-installer/docs/how-to/update-chezmoi-content.md).
