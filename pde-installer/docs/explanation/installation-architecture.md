# Installation Architecture

The repository describes full and terminal PDE profiles. `install` reconciles
the selected profile; `update` reconciles the saved profile. Terminal skips
full-only components, while terminal can expand to full through `install
--profile full`. `config` applies the saved profile's chezmoi configuration.

## Dependency Order

The full order is:

1. Install missing Ubuntu dependencies with apt.
2. Check the host compiler, build tools, Ubuntu release, repository metadata,
   and writable destinations.
3. Download and activate the existing amd64-only direct tmux 3.7b binary.
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
