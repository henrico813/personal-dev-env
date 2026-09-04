# Tool Selection

The installer assigns tools by this policy:

1. Use apt for Ubuntu operating-system dependencies.
2. Use Aqua for standalone command-line binaries.
3. Use direct upstream releases for exact runtimes or layouts Aqua does not
   support.
4. Use npm for npm-native tools.
5. Build repository applications locally.
6. Use chezmoi for home configuration and external config content.
7. Build from source only when no suitable package or binary is available.

The Aqua registry also selects compatible artifacts, including Yazi's musl
build on Ubuntu 22.04.

tmux uses a pinned, verified static release installed under `HOME`.

A profile chooses which components to install. It does not change how those
components are installed. The full profile contains the complete development
environment. The terminal profile leaves out editors, programming toolchains,
graphical terminal configuration, and AI tools.
