# Update Local Builds

The full-profile local backend builds `planner`, `opencode-inline-shim`,
`vibe`, and `pde-gh-write` (from `pdev2/`, also linked as
`pde-pr-approve`).

1. Change the application source in its repository directory.
2. If an application is added or removed, update the build specifications in
   `internal/builds/builds.go` and the inventory in
   `internal/manifest/manifest.go`.
3. Run:

   ```bash
   go test ./...
   go run . install --dry-run --repo-root ..
   go run . install --repo-root ..
   ```

The installer hashes regular source files, excluding `.git` and `target`
directories. It rebuilds the local applications when inputs or managed Go and
Rust toolchain versions change. It builds `blink.cmp` later, after chezmoi has
installed the plugin tree.

There is no local-build-only command.
