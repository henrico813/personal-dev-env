# Update Aqua Tools

## Update a Managed Tool

1. Change its version in
   `chezmoi/dot_config/aquaproj-aqua/aqua.yaml` from the repository root.
2. Decide whether the tool belongs in the `terminal` profile. If it does, also
   change `chezmoi/dot_config/aquaproj-aqua/aqua-terminal.yaml`. Language
   servers stay only in `aqua.yaml` for the `full` profile.
3. Change the matching Aqua entry and its profile in
   `pde-installer/internal/manifest/manifest.go`.
4. Update
   `chezmoi/dot_config/aquaproj-aqua/aqua-checksums.json` with trusted
   checksums for the new package.
   The Yazi package supplies both `yazi` and `ya`; update both manifest entries
   when changing it.
5. When updating chezmoi, also update its version in
   `pde-installer/internal/chezmoi/apply.go`.
6. From `pde-installer/`, run:

   ```bash
   go test ./...
   go run . update --dry-run --repo-root ..
   ```

## Update Aqua Itself

Update Aqua's version in `pde-installer/internal/manifest/manifest.go`. Update
its amd64 and arm64 archive checksums in
`pde-installer/internal/aqua/aqua.go`.

The installer hashes `aqua.yaml` and `aqua-terminal.yaml`. A changed hash causes
it to stage a complete Aqua root, install the pinned packages for the saved
profile, verify versions, and replace the old root.

There is no Aqua-only update. Reconcile the saved profile:

```bash
go run . update --repo-root ..
```

See [component metadata](../reference/component-metadata.md).
