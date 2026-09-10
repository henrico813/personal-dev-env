# Update Chezmoi Content

1. Edit files under the repository's `../chezmoi/` source directory.
2. For an external archive, update its URL and SHA-256 value in
   `../chezmoi/.chezmoiexternal.toml.tmpl`. Every external must have a checksum.
3. When changing files under `../ai/`, update the matching SHA-256 value in
   `../chezmoi/.chezmoiexternal.toml.tmpl`.
4. Preview only configuration changes:

   ```bash
   go run . config --dry-run --repo-root ..
   ```

5. Apply them:

   ```bash
   go run . config --repo-root ..
   ```

Use `config` when only managed home configuration changed. The installer passes
`PDE_PROFILE` when rendering profile-aware templates, so use it rather than
applying chezmoi directly. For command selection, prerequisites, and examples,
run `pde-installer config --help` or `pde-installer update --help`.
