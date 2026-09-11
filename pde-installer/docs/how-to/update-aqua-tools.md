# Update Aqua Tools

## Update a Managed Tool

1. Change the terminal-owned pin in both
   `../chezmoi/dot_config/aquaproj-aqua/aqua.yaml` and
   `../chezmoi/dot_config/aquaproj-aqua/aqua-terminal.yaml`.
2. Change the matching Aqua entry in `internal/manifest/manifest.go`.
3. Update both
   `../chezmoi/dot_config/aquaproj-aqua/aqua-checksums.json` and
   `../chezmoi/dot_config/aquaproj-aqua/aqua-terminal-checksums.json` with
   trusted checksums for the new package. Use the matching environment paths:

   ```bash
   AQUA_GLOBAL_CONFIG="$PWD/../chezmoi/dot_config/aquaproj-aqua/aqua.yaml" \
   AQUA_CHECKSUMS_PATH="$PWD/../chezmoi/dot_config/aquaproj-aqua/aqua-checksums.json" \
   aqua update-checksum -a -prune
   AQUA_GLOBAL_CONFIG="$PWD/../chezmoi/dot_config/aquaproj-aqua/aqua-terminal.yaml" \
   AQUA_CHECKSUMS_PATH="$PWD/../chezmoi/dot_config/aquaproj-aqua/aqua-terminal-checksums.json" \
   aqua update-checksum -a -prune
   ```

   Yazi has one Aqua package entry, and that package supplies the two inventory
   executables `yazi` and `ya`; update that entry in both Aqua manifests when
   changing it.
4. Keep `gopls` and `lua-language-server` in the full manifest only. When
   updating chezmoi, also update its version in `internal/chezmoi/apply.go`.
5. Run:

   ```bash
   go test ./...
   go run . install --dry-run --repo-root ..
   ```

## Update Aqua Itself

Update Aqua's version in `internal/manifest/manifest.go`. Update its amd64 and
arm64 archive checksums in `internal/aqua/aqua.go`.

The installer hashes the selected profile's Aqua manifest and matching checksum
file. A changed hash causes it to stage an Aqua root for that profile, install
its pinned packages, verify versions, and replace the old root.

There is no Aqua-only command. Run install to reconcile the saved selection:

```bash
go run . install --repo-root ..
```

See [component metadata](../reference/component-metadata.md).
