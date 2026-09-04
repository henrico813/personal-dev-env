# Update Aqua Tools

## Update a Managed Tool

1. Change its version in
   `../chezmoi/dot_config/aquaproj-aqua/aqua.yaml`.
2. Change the matching Aqua entry in `internal/manifest/manifest.go`.
3. Update
   `../chezmoi/dot_config/aquaproj-aqua/aqua-checksums.json` with trusted
   checksums for the new package.
   The Yazi package supplies both `yazi` and `ya`; update both manifest entries
   when changing it.
4. Run:

   ```bash
   go test ./...
   go run . update --dry-run --repo-root ..
   ```

## Update Aqua Itself

Update the Aqua manifest version and the amd64 and arm64 archive checksums in
`internal/aqua/aqua.go`.

The installer hashes both Aqua configuration files. A changed hash causes it to
stage a complete Aqua root, install all pinned packages, verify versions, and
replace the old root.

There is no Aqua-only update. Run the full reconciliation:

```bash
go run . update --repo-root ..
```

See [component metadata](../reference/component-metadata.md).
