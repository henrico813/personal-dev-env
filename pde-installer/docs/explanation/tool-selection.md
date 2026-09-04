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

The installer downloads a pinned static tmux binary and verifies its checksum.
PDE no longer builds tmux or uses pkgsrc on the target host.

Users do not choose a policy during a run. The repository assigns every item an
owner, and `install` or `update` reconciles all owners. Only `config` exposes a
supported subset.
