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

Updating tmux uses two pull requests so the installer only refers to an
immutable release.

1. In the first pull request, update the tmux version, source checksum, and
   revision marker in `.github/scripts/build-static-tmux.sh`. Update the release
   tag and revision marker in `.github/workflows/release-static-tmux.yml`. Keep
   the build image and every build package pinned.
2. Merge that pull request and run the workflow from `main`. It builds Linux
   amd64 and arm64 archives, checks that each binary is static and works, and
   publishes a release such as `tmux-3.6a-pde.1`.
3. Download both archives and `SHA256SUMS` into one directory. Run
   `sha256sum -c SHA256SUMS`. Do not replace files in an existing release.
   Increase the revision marker for another build.
4. In the second pull request, copy each checksum exactly from its line in
   `SHA256SUMS` into `internal/tmux/tmux.go`. Update the release URLs and set the
   same version in `internal/manifest/manifest.go`.
5. Run the tests and preview shown above. Confirm the installer downloads the
   archive for the current architecture, verifies its checksum, and installs
   the static release under `HOME`.

There is no Ubuntu-only or tmux-only command. `update` reconciles the saved
profile.
