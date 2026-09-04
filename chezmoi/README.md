# Chezmoi Source

This directory owns PDE home configuration and checksummed external content.
The installer passes the selected profile to chezmoi. The `terminal` branch
skips full-only files and downloads. The `full` branch applies the complete
source. `pde-installer config` uses the saved profile.

Edit files here using chezmoi source names. Add remote archives to
`.chezmoiexternal.toml.tmpl` with a SHA-256 checksum.

Preview and apply configuration changes from `pde-installer/`:

```bash
go run . config --dry-run --repo-root ..
go run . config --repo-root ..
```

Run chezmoi through `pde-installer`, not directly. The installer snapshots
changed targets before apply. A failed apply restores
the snapshot. Modifier scripts preserve selected user-owned settings.

See the [maintenance guide](../pde-installer/docs/how-to/update-chezmoi-content.md).
