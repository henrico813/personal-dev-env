# Update npm Tools

1. Change the exact version in `package.json`.
2. Regenerate `package-lock.json` with lockfile version 3.
3. Copy the package's new SHA-512 integrity value into its entry in
   `internal/npm/npm.go`.
4. Set the same version in `internal/manifest/manifest.go`.
5. Run:

   ```bash
   go test ./...
   go run . update --dry-run --repo-root ..
   go run . update --repo-root ..
   ```

The installer requires exactly the four declared top-level packages. It uses
the managed Node.js and npm release, runs `npm ci` from the complete lock, runs
required install scripts in staging, verifies each command's version, and then
activates the prefix and launchers.

npm owns npm-native command-line tools. Individual npm tools cannot be selected
for update.
