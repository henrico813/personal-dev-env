# Installation Architecture

The coordinator reads the profile and runs the managers needed for it. The
`full` profile runs every stage. The `terminal` profile skips toolchains, npm,
fonts, repository builds, Neovim, and AI configuration.

## Dependency Order

The full order is:

1. Install missing Ubuntu dependencies with apt.
2. Check the host compiler, build tools, Ubuntu release, repository metadata,
   and writable destinations.
3. Install and activate the static tmux release.
4. Install Aqua and all Aqua tools.
5. Install direct-release tools, including Node.js, Go, and Rust.
6. Install npm tools with the managed npm.
7. Install direct-release fonts and run `fc-cache`.
8. Build repository applications with the managed toolchains.
9. Migrate legacy PDE and Neovim configuration.
10. Apply the chezmoi source.
11. Build and verify the `blink.cmp` native library.
12. Commit journals and remove backups.

This order gives each stage the tools and files it needs. A later failure rolls
back completed journaled stages in reverse order.

Before any non-dry-run mutation, the installer takes a per-user lock and
recovers old journals. See [journals and recovery](journals-and-recovery.md).
