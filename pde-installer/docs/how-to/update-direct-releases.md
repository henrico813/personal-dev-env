# Update Direct Releases

Direct releases contain runtimes, Neovim, Keychain, and fonts.

## Tools and Runtimes

1. Change the version in `internal/manifest/manifest.go`.
2. Update architecture checksums in `internal/direct/tools.go`. Keychain uses
   one architecture-independent checksum.
3. If the upstream archive name, URL, directory layout, or version output
   changed, update its `Tool` entry there too.
4. Run:

   ```bash
   go test ./...
   PDE_TEST_OFFICIAL_TOOLS=1 go test ./internal/direct
   go run . update --dry-run --repo-root ..
   ```

## Fonts

1. Change each font version in `internal/manifest/manifest.go`.
2. Update archive names and checksums in `internal/direct/direct.go`.
3. Run the tests and preview above.

The backend verifies SHA-256 values before activation. It uses direct releases
for exact runtimes and layouts that other supported managers do not provide.
There is no direct-release-only command; `update` reconciles all backends.
