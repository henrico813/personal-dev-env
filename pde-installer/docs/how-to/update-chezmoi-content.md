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
installer passes `PDE_PROFILE` and `PDE_COLOR_PROFILE` when rendering
selection-aware templates, so use it rather than applying chezmoi directly.
Shared palette and application-theme data belongs in
`../chezmoi/.chezmoidata.json`; keep the supported names aligned with
`internal/colorprofile`. For examples, run `pde-installer install --help`.

Removing a managed file only stops chezmoi from writing it; it does not delete
the live copy. Add the home-relative target path to `../chezmoi/.chezmoiremove`,
the list of files chezmoi deletes from your home directory. When the file has a
live side effect such as an enabled systemd unit, add a `run_once_before_<name>`
script; chezmoi runs these once, before it applies changes, so the script can
reverse the effect before the file is removed. Do not also list the target in
`.chezmoiignore`; ignored targets are skipped by the `.chezmoiremove` list.
