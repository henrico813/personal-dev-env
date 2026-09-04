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

`config` migrates legacy PDE config, then applies the chezmoi branch for the
saved profile. The `terminal` branch skips full-only files and downloads. The
`full` branch applies the complete source. A dry run uses read-only status and
diff commands without refreshing externals or running scripts.

On a fresh system with no `config.json` or legacy `paths.env`, the profile is
`full`. If either file is nonempty but has no profile, `config` stops. Add
`"profile": "full"` or `"profile": "terminal"` to
`~/.config/pde/config.json`, then rerun the command.

Use `update` instead when other component metadata also changed. `update` uses
the saved profile before applying chezmoi.
