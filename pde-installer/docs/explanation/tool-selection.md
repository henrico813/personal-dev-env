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

The installer downloads and verifies the existing amd64-only direct tmux
3.7b binary from its pinned release.

Profile selection determines which inventory and owners are reconciled.
`install` uses its selected profile, while `update` uses the saved profile.
Terminal skips full-only direct releases, runtimes, npm tools, builds, fonts,
and editor/AI configuration. `config` applies the saved profile's chezmoi
configuration.
