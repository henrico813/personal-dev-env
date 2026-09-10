# Update Chezmoi Content

1. Edit files under the repository's `../chezmoi/` source directory.
2. For an external archive, update its URL and SHA-256 value in
   `../chezmoi/.chezmoiexternal.toml.tmpl`. Every external must have a checksum.
3. Preview only configuration changes:

   ```bash
   go run . config --dry-run --repo-root ..
   ```

4. Apply them:

   ```bash
   go run . config --repo-root ..
   ```

`config` applies the chezmoi source for the saved profile. It migrates legacy
PDE config first. Its dry run uses read-only status and diff commands without
refreshing externals or running scripts. The installer passes `PDE_PROFILE` when
rendering profile-aware templates, so use the installer rather than applying
chezmoi directly.

Use `update` instead when other component metadata also changed. `update` reconciles the saved profile's backends before applying chezmoi.
