# Update Chezmoi Content

1. Edit files under the repository's `../chezmoi/` source directory.
2. For an external archive, update its URL and SHA-256 value in
   `../chezmoi/.chezmoiexternal.toml.tmpl`. Every external must have a checksum.
3. When changing files under `../ai/`, update the matching SHA-256 value in
   `../chezmoi/.chezmoiexternal.toml.tmpl`.
4. Preview the complete saved-selection reconciliation:

   ```bash
   go run . install --dry-run --repo-root ..
   ```

5. Apply them:

   ```bash
   go run . install --repo-root ..
   ```

Every install reconciles tools as well as managed home configuration. The
installer passes `PDE_PROFILE` when rendering selection-aware templates, so use
it rather than applying chezmoi directly. For selection, prerequisites, and
examples, run `pde-installer install --help`.
