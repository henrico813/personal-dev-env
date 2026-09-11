# Tool Selection

The installer assigns tools by this policy:

1. Use apt for Ubuntu operating-system dependencies.
2. Use Aqua for standalone command-line binaries.
3. Use direct upstream releases for exact runtimes or layouts Aqua does not
   support.
4. Use npm for npm-native tools.
5. Build repository applications locally.
6. Use chezmoi for home configuration and external config content.

The Aqua registry also selects compatible artifacts, including Yazi's musl
build on Ubuntu 22.04.

The installer downloads and verifies tmux, the existing amd64-only direct
binary, version 3.7b from its pinned release.

The install selection determines which inventory and owners are reconciled.
An explicit `install terminal` or `install full` saves its selection; a bare
install reuses saved or legacy selection.
Terminal skips full-only direct releases, runtimes, npm tools, builds, fonts,
and editor/AI configuration. Switching from full to terminal does not remove
full-only artifacts. Every install also applies the selected managed home
configuration.
