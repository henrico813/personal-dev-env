# Update Local Builds

The local backend builds repository applications: `planner`,
`opencode-inline-shim`, `vibe`, `pde-gh-write` (also linked as
`pde-pr-approve`), `pde-comment-review`, `skill-lessons` and
`pde-comment-checker`. Each of the last two has a `build.py` that packs
it into one file run by `/usr/bin/python3`. Client builds only
`skill-lessons`, and terminal builds no local applications.

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
directories. For `skill-lessons` and `pde-comment-checker` it hashes only
`build.py` and the regular `.py` files under `src/`, so caches and tests do not
count. It rebuilds all seven applications when inputs or managed Go and Rust
toolchain versions change. A Python upgrade does not trigger a rebuild or a
check; after upgrading `/usr/bin/python3`, run `pde-comment-checker version` and
`skill-lessons --version` yourself. If it fails, the code needs a fix for
that Python; a rebuild alone won't help, since the packed file is plain
source. It builds `blink.cmp` later, after chezmoi has installed the
plugin tree.

There is no local-build-only command.
