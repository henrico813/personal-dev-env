# Update npm Tools

1. Change the exact version in `package.json`.
2. Regenerate `package-lock.json` with lockfile version 3.
3. Copy the package's new SHA-512 integrity value into its entry in
   `internal/npm/npm.go`.
4. Set the same version in `internal/manifest/manifest.go`.
5. If changing OpenCode or its Claude adapter, synchronize the exact pins in
   `chezmoi/dot_config/opencode/modify_opencode.json` and its modifier tests.
6. Run:

   ```bash
   go test ./...
   go run . install --dry-run --repo-root ..
   go run . install --repo-root ..
   ```

The installer requires exactly the five declared top-level packages. It uses
the managed Node.js and npm release, runs `npm ci` from the complete lock, runs
required install scripts in staging, verifies each command's version, and then
activates the prefix and launchers.

npm owns npm-native command-line tools. Individual npm tools cannot be selected
for reconciliation.
