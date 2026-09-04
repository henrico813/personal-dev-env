# Update Ubuntu Dependencies and tmux

Ubuntu dependencies and tmux have separate metadata.

## Ubuntu Dependencies

1. Edit the package list in `internal/ubuntu/ubuntu.go`.
2. Keep the inventory entries in `internal/manifest/manifest.go` aligned with
   packages that should appear in `pde-installer list`.
3. Run the tests and preview:

   ```bash
   go test ./...
   go run . update --dry-run --repo-root ..
   ```

The backend checks `dpkg-query`. If any required package is missing, it runs
`sudo apt-get update` and one `sudo apt-get install -y --no-install-recommends`
command. It does not upgrade a package that is already installed.

## tmux

1. Update `version`, `archiveURL`, and `archiveSHA256` in
   `internal/tmux/tmux.go`.
2. Set the same version on the tmux entry in
   `internal/manifest/manifest.go`.
3. Run the tests and preview shown above.

The installer downloads the pinned amd64 binary from
`mjakob-gh/build-static-tmux` and verifies its SHA-256 checksum. Do not use the
latest-release URL. Pin a versioned asset and its checksum.

There is no Ubuntu-only or tmux-only command. `update` reconciles everything.
