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
the snapshot. Modifier scripts preserve selected user-owned settings.

See the [maintenance guide](../pde-installer/docs/how-to/update-chezmoi-content.md).
