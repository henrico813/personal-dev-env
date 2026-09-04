# Update Chezmoi Content

1. Edit files under the repository's `chezmoi/` source directory.
2. For an external archive, update its URL and SHA-256 value in
   `chezmoi/.chezmoiexternal.toml`. Every external must have a checksum.
3. Preview only configuration changes:

   ```bash
   go run . config --dry-run --repo-root ..
   ```

4. Apply them:

   ```bash
   go run . config --repo-root ..
   ```

`config` is the only supported subset operation. It migrates legacy PDE config,
then applies the complete chezmoi source. Its dry run uses read-only status and
diff commands without refreshing externals or running scripts.

Use `update` instead when other component metadata also changed. `update`
reconciles every backend before applying chezmoi.
