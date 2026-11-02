# Update Local Builds

The local backend builds repository applications: `planner`,
`opencode-inline-shim`, `vibe`, `pde-gh-write` (also linked as
`pde-pr-approve`), `pde-comment-review`, `pde-comment-review-next`,
`skill-lessons`, `pde-comment-checker` and `prs`. `skill-lessons` and
`pde-comment-checker` each have a `build.py` that packs it into one file run
by `/usr/bin/python3`. Client builds only `skill-lessons`, and terminal builds
no local applications.

The Rust `pde-comment-review-next` command runs only `help` for now.
It installs beside the current review tool and checker without replacing them.
The installer builds it with Cargo and checks it with `help` after activation.
A failed check restores the previous tools through the installer's journal.

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
count. It rebuilds all nine applications when inputs or managed Go and Rust
toolchain versions change. A Python upgrade does not trigger a rebuild or a
check; after upgrading `/usr/bin/python3`, run `pde-comment-checker version` and
`skill-lessons --version` yourself. If it fails, the code needs a fix for
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
