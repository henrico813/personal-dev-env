# Configure the OpenCode Goog provider

OpenCode v2 reads the Goog provider from `providers.goog` in
`~/.config/opencode/opencode.json`. The installer manages that entry on every
run: it removes the hand-written legacy `provider.ollama` block, drops the
`provider` key when nothing else remains in it, and merges the v2 provider
shape.

The provider keeps three pieces of configuration:

- `name "Goog"` and `package "@opencode/ai/providers/openai-compatible"`,
  managed by the repository;
- `settings.baseURL`, the non-secret endpoint. It is preserved from your
  local `opencode.json` (or from the legacy `provider.ollama` value on a
  first run) and is not stored in the repository. `https` is preferred;
  `http` is allowed only for a trusted LAN endpoint;
- `settings.apiKey "{file:~/.config/opencode/goog.key}"`, a reference to a
  local key file. The key itself never enters the configuration, the
  repository, or a command line.

## Create the key file

1. Get an OpenCode-specific LiteLLM key. Vibe uses a separate key in
   `~/.config/vibe/goog.env`; see the last section before reusing anything.
2. Create the file without putting the key in your shell history:

   ```bash
   IFS= read -rs OPENCODE_KEY
   umask 077
   printf '%s\n' "$OPENCODE_KEY" > ~/.config/opencode/goog.key
   ```

   `read -rs` hides the entry and keeps it in the current shell, and the
   `printf` builtin writes it straight to the file. Do not assign the key
   inline on the command line (`OPENCODE_KEY='...'`): that leaks it into
   shell history. A key also reaches another process's argv only when it is
   passed to an external command such as `/usr/bin/printf` or `echo` rather
   than the shell builtin; the `read -rs` plus `printf` builtin route avoids
   both. Opening `~/.config/opencode/goog.key` in an editor and pasting the
   key works just as well.

   The file holds exactly one line (the key plus a trailing newline). The
   installer refuses to read a key file that is a symlink, not a regular
   file, not owned by you, or group- or world-writable, and a file that
   fails any of those checks makes the run keep the configured models and
   warn once on stderr.

## Apply and verify

1. Run a normal install. With the key file in place, the installer discovers
   the endpoint's models at apply time, writes the `models` map keyed by
   bare ID, and removes the legacy `provider.ollama` block:

   ```bash
   go run . install --repo-root ..
   ```

2. Confirm the provider and select a model as `goog/<id>`, for example
   `goog/qwen3.8`:

   ```bash
   opencode models | grep '^goog/'
   opencode run --model goog/qwen3.8 'say hi'
   ```

   No environment variable is needed for OpenCode; the key file supplies the
   credential.

If the endpoint is unreachable or answers with an error, the run preserves
the existing `models` map and prints one warning on stderr. On a first run
with no prior models, the legacy provider stays in place until a run
discovers a non-empty model map.

## Vibe keeps its own key

Vibe reads its Goog credentials from `~/.config/vibe/goog.env`
(`GOOG_BASE_URL`, `GOOG_API_KEY`). That is a deliberately separate LiteLLM
key from OpenCode's `~/.config/opencode/goog.key`, so each tool's usage stays
attributable. Do not copy one key into the other tool's file.
