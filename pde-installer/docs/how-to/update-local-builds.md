# Update Local Builds

The local backend builds repository applications: `planner`,
`opencode-inline-shim`, `vibe`, `pde-gh-write` (also linked as
`pde-pr-approve`), `pde-comment-review`, and `skill-lessons`, which
`skill-lessons/build.py` packs into one file run by `/usr/bin/python3`.

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
directories. For `skill-lessons` it hashes only `build.py` and the regular
`.py` files in `src/skill_lessons/`, so Python caches and test runs do not
count. It rebuilds all six applications when inputs or managed Go and Rust
toolchain versions change. A Python upgrade does not trigger a rebuild or a
`--version` check; after upgrading `/usr/bin/python3`, run
`skill-lessons --version` yourself. If it fails, the code needs a fix for
that Python; a rebuild alone won't help, since the packed file is plain
source. It builds `blink.cmp` later, after chezmoi has installed the
plugin tree.

There is no local-build-only command.
