# Component Metadata

`internal/manifest/manifest.go` is the inventory used by validation and
`pde-installer list`. It assigns one owner and, where required, one version to
each item. It is not an uninstall list.

| Component | Primary metadata |
|---|---|
| Ubuntu dependencies | `internal/ubuntu/ubuntu.go` |
| tmux | Existing amd64-only direct 3.7b binary from `mjakob-gh/build-static-tmux` and checksum in `internal/tmux/tmux.go`; manifest version |
| Aqua | `internal/aqua/aqua.go`; manifest version and architecture checksums |
| Aqua tools | `../chezmoi/dot_config/aquaproj-aqua/`, manifest versions, and the chezmoi version in `internal/chezmoi/apply.go` |
| Direct tools | `internal/direct/tools.go`; manifest versions and amd64/arm64 checksums |
| Fonts | `internal/direct/direct.go`; manifest versions |
| npm tools | `package.json`, `package-lock.json`, `internal/npm/npm.go`, and manifest versions |
| Repository builds | `internal/builds/builds.go`; manifest inventory |
| `blink.cmp` native build | pinned URL and checksum in `internal/builds/builds.go` |
| Home configuration | repository `chezmoi/` source and `.chezmoiexternal.toml.tmpl` checksums |
| Color profiles | names in `internal/colorprofile`; palettes and application themes in `../chezmoi/.chezmoidata.json` |

Pins repeated across files must agree. Profile selection limits the terminal
inventory, the Aqua manifest and checksum files used, and the chezmoi output.
Validation catches duplicate inventory names, missing required pins, incomplete
npm locks, missing chezmoi source data, and externals without SHA-256 fields.
Backend reconciliation performs further checksum and installed-version checks.
Tests require the Go names and Chezmoi data keys to match and validate complete
normal, bright, interface, and application-theme fields for every profile.

See the matching [how-to guide](../README.md#how-to-guides) before changing
metadata.
