# Update Local Builds

The local backend builds repository applications: `planner`,
`opencode-inline-shim`, `vibe`, `pde-gh-write` (also linked as
`pde-pr-approve`), `glimmer`, `hub`, `skill-lessons`, `hub-py` and `prs`.
`skill-lessons` and `hub-py`, the deprecated Python hub in `hub/`, each have a
`build.py` that packs them into one file run by `/usr/bin/python3`. Client
builds only `skill-lessons` and `hub-py`, and terminal builds no local
applications.

The installer builds `glimmer` from the Rust source in `glimmer/`
with Cargo and checks it with `help` after activation. It builds
`hub`, the Rust hub, from `hub-rs/` the same way and checks it
with `--version`.
A failed check restores the previous tools through the installer's journal.

When a build is renamed, list its old and new names in `renamedBuilds` in
`internal/builds/builds.go`. Once a profile builds only the new name, the
installer deletes `~/.local/bin/<old name>`, so `hub-next` and, on client, the
old Python `hub` do not stay behind.

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
directories. For `skill-lessons` and `hub-py` it hashes only `build.py` and the
regular `.py` files under `src/`, so caches and tests do not count. It rebuilds
all nine applications when inputs or managed Go and Rust toolchain versions
change. A Python upgrade does not trigger a rebuild or a check; after upgrading
`/usr/bin/python3`, run `skill-lessons --version` and `hub-py --version` yourself.
If one fails, the code needs a fix for
that Python; a rebuild alone won't help, since the packed file is plain
source. It builds `blink.cmp` later, after chezmoi has installed the
plugin tree.

## Wallpaper players

A machine with a desktop also builds that desktop's animated wallpaper
player, since no Ubuntu package provides one: `xwinwrap` for i3, and `mpvpaper`
with `mpvpaper-holder` for Sway. They build on every profile, terminal and
client included, and their compilers and headers come from the desktop's apt
groups in `internal/profile/profiles.yaml`.

Each player is a pinned source archive in `wallpaperPlayers` in
`internal/builds/builds.go`. The installer downloads it, checks its SHA-256,
builds it under `~/.local/state/pde` and puts the binaries in `~/.local/bin`.
`~/.local/state/pde/<player>.json` records the archive's SHA-256. A rerun
skips the build when that matches and `mpvpaper -h` exits 0, so an apt update
that breaks mpvpaper's libmpv link rebuilds it. `xwinwrap` has no such check
and rebuilds when its pin changes or its binary is missing or not executable.

To change a pin, set the archive URL to the new commit, download it, and
replace the SHA-256 with the output of `sha256sum`. For `mpvpaper`, also
update the tag comment above its URL. If the build needs new packages, add
them to the desktop's group in `profiles.yaml`. Then build in a fresh Ubuntu
24.04 container with only the desktop's apt groups installed, and run
`install i3` or `install sway` on a desktop machine.

There is no local-build-only command.
