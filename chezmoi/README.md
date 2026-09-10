# Chezmoi Source

This directory owns PDE home configuration and checksummed external content.
`pde-installer install` and `update` apply it during reconciliation.
`pde-installer config` runs only config migration and this source. The
installer passes `PDE_PROFILE=full|terminal` while rendering it; apply through
the installer so the profile-specific ignore and external templates match.

Edit files here using chezmoi source names. Add remote archives to
`.chezmoiexternal.toml.tmpl` with a SHA-256 checksum.

Preview and apply configuration changes from `pde-installer/`:

```bash
go run . config --dry-run --repo-root ..
go run . config --repo-root ..
```

The installer snapshots changed targets before apply. A failed apply restores
the snapshot. Modifier scripts preserve selected user-owned settings.

See the [maintenance guide](../pde-installer/docs/how-to/update-chezmoi-content.md).
