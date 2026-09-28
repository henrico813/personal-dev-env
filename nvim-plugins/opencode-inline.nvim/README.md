# opencode-inline.nvim

CodeCompanion inline editing backed by OpenCode 2.

CodeCompanion inline interactions accept only HTTP adapters, so OpenCode's ACP
adapter cannot serve them. This plugin registers an OpenAI-compatible
CodeCompanion adapter backed by `opencode-inline-shim`, a small Go server that
translates each inline request into a temporary OpenCode session:

```text
CodeCompanion inline -> POST 127.0.0.1:<port>/v1/chat/completions
  -> POST /api/session -> POST /api/session/{id}/generate
  -> validate the edit JSON -> DELETE /api/session/{id}
```

## Requirements

- Neovim 0.10 or newer
- [codecompanion.nvim](https://github.com/olimorris/codecompanion.nvim) and
  plenary.nvim
- OpenCode 2.0.18 (`opencode` on `$PATH` for auto-start). Later 2.x releases
  may drop the session generate route.
- Go 1.21 or newer to build the shim
- Linux `ps`, `ss`, and `kill` for shim process management

## Install

Build the shim into the plugin's `bin/` directory after installing or updating:

```bash
make build
```

The plugin lives in a subdirectory of the PDE repository, so point the plugin
manager at a local checkout. lazy.nvim:

```lua
{
  dir = "~/src/personal-dev-env/nvim-plugins/opencode-inline.nvim",
  build = "make build",
  dependencies = { "olimorris/codecompanion.nvim" },
}
```

Native packages:

```bash
ln -s ~/src/personal-dev-env/nvim-plugins/opencode-inline.nvim \
  ~/.config/nvim/pack/plugins/start/opencode-inline.nvim
make -C ~/.config/nvim/pack/plugins/start/opencode-inline.nvim build
```

Install the bundled OpenCode agent, or set `agent = ""`:

```bash
cp agents/inline.md ~/.config/opencode/agents/inline.md
```

## Configure

```lua
local inline = require("opencode-inline")

inline.setup({
  opencode_url = "http://127.0.0.1:4199",
  port = 4141,
  model = nil, -- "provider/model[#variant]", e.g. "openrouter/z-ai/glm-5.3-prime#high"
  agent = "inline",
  password = nil,
  cmd = nil,
})

require("codecompanion").setup({
  adapters = { http = { opencode_inline = inline.adapter } },
  interactions = { inline = { adapter = "opencode_inline" } },
})

vim.keymap.set({ "n", "x" }, "<leader>pi", inline.prompt, { desc = "Inline prompt" })
vim.keymap.set("n", "<leader>pM", inline.select_model, { desc = "Select inline model" })
```

| Option | Default | Meaning |
|---|---|---|
| `opencode_url` | `http://127.0.0.1:4199` | OpenCode 2 server. A loopback URL with an explicit port is started on demand with `opencode serve`. |
| `port` | `4141` | Loopback port for the shim. |
| `model` | `nil` | Inline model. `nil` uses OpenCode's current default. |
| `agent` | `"inline"` | OpenCode agent for inline sessions. OpenCode rejects unknown agents; `""` uses its default agent. |
| `password` | `nil` | OpenCode server password. `nil` uses `$OPENCODE_SERVER_PASSWORD`, then a generated password stored in `stdpath("state")/opencode-inline/server-password`. |
| `cmd` | `nil` | Shim executable. `nil` uses `bin/opencode-inline-shim`, then `$PATH`. |

OpenCode 2 servers require HTTP basic authentication. The shim sends
`OPENCODE_SERVER_USERNAME` (default `opencode`) and the password above, and
passes both to any server it starts. A running server started with another
password is reported instead of replaced; stop it or configure its password.

Options are read by `setup()`. Run `:OpenCodeInlineRestart` after changing
them so a reused shim picks up the new values.

## API

| Function | Behavior |
|---|---|
| `adapter()` | CodeCompanion HTTP adapter factory. |
| `prompt()` | Start the shim, ask for an instruction, and run CodeCompanion inline on the buffer or visual selection. |
| `select_model()` | Pick an inline model for this Neovim session from OpenCode's ACP model list. |
| `start(on_ready?)` | Start or reuse a healthy shim. |
| `restart()` | Replace the shim. Also available as `:OpenCodeInlineRestart`. |
| `status()` | Spinner text while inline requests are pending, otherwise `""`. |

The shim is detached and reused across Neovim sessions. Check it directly with
`opencode-inline-shim --healthcheck --port <port>`.

## Development

```bash
make check
```
