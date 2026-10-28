# Installation Architecture

The repository describes terminal, client, and full PDE selections.
`install terminal`, `install client`, and `install full` save an explicit
selection. Bare `install` reconciles the saved selection or infers full from
legacy installer state; it errors when neither exists. Terminal skips full-only
components; client installs everything except what needs Go, Rust, or a
compiled local build, and only five of the shared skills. Switching between
selections changes future reconciliation, but
switching from full to terminal does not uninstall full-only artifacts. Client
refuses to install over release tools it would delete, such as Go left by a
past full install. The exception is PR write approval: chezmoi removes
`pde-gh-write`, `pde-pr-approve`, and the `gh` guard on terminal and client
machines. Every install reconciles tools and
managed home configuration.

Color selection is an independent per-machine value in the same config file.
Install resolves both selections after journal recovery and passes them to
Chezmoi. Missing color state resolves to Tokyo Night. The migration journal
activates the updated config before Chezmoi runs, restores the previous config
on a later failure, and makes the update permanent when installation commits.

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
8. Build repository applications with the managed toolchains, and pack
   `skill-lessons` and `pde-comment-checker` with `/usr/bin/python3`.
9. Migrate legacy PDE and Neovim configuration.
10. Install the inline plugin runtime.
11. Apply the chezmoi source.
12. Build and verify the `blink.cmp` native library.
13. Commit journals and remove backups.

Terminal skips steps 5 to 8, 10, and 12. Client skips step 2's compiler
checks, installs no Go, Rust, Keychain, or Moshi Hook in step 5, only packs
`skill-lessons` in step 8, and installs blink.cmp without its native matcher in
step 12, so it compiles nothing. Before step 1, client refuses to run if step 5
would delete installed release tools.

This order gives each stage the tools and files it needs. A later failure rolls
back completed journaled stages in reverse order.

Before any non-dry-run mutation, the installer takes a per-user lock and
recovers old journals. See [journals and recovery](journals-and-recovery.md).
