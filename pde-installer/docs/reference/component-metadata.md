# Component Metadata

`internal/manifest/manifest.go` is the inventory used by validation and
`pde-installer list`. It assigns one owner and, where required, one version to
each item. It is not an uninstall list.

| Component | Primary metadata |
|---|---|
| Ubuntu dependencies | `internal/ubuntu/ubuntu.go` |
| tmux | `internal/tmux/tmux.go`; manifest version |
| Aqua | `internal/aqua/aqua.go`; manifest version and architecture checksums |
| Aqua tools | `../chezmoi/dot_config/aquaproj-aqua/`, manifest versions, and the chezmoi version in `internal/chezmoi/apply.go` |
| Direct tools | `internal/direct/tools.go`; manifest versions and amd64/arm64 checksums |
| Fonts | `internal/direct/direct.go`; manifest versions |
| npm tools | `package.json`, `package-lock.json`, `internal/npm/npm.go`, and manifest versions |
| Repository builds | `internal/builds/builds.go`; manifest inventory |
| `blink.cmp` native build | pinned URL and checksum in `internal/builds/builds.go` |
| Home configuration | repository `chezmoi/` source and `.chezmoiexternal.toml` checksums |

Pins repeated across files must agree. Validation catches duplicate inventory
names, missing required pins, incomplete npm locks, missing chezmoi source data,
and externals without SHA-256 fields. Backend reconciliation performs further
checksum and installed-version checks.

See the matching [how-to guide](../README.md#how-to-guides) before changing
metadata.
