# nvim

A hand-rolled Neovim config using the native `pack/` loader instead of a plugin manager. Explicit by design — every plugin and keymap lives in a file you wrote.

Launch it with:

```bash
nvim
```

The installer (`pde minimal` and `pde full`) clones all plugins into `~/.config/nvim/pack/plugins/start/` and symlinks this directory into place. The live config is a symlink back here, so edits take effect immediately — no copying, no sync step.

---

## Mental model

Just enough Neovim vocabulary to read your own config.

**Buffer, window, tab.** A *buffer* is a file loaded into memory. A *window* is a viewport showing a buffer. A *tab* is a layout of windows. One buffer can be shown in multiple windows; one tab can hold many windows. You can have dozens of buffers open with only one window visible — `:ls` lists all buffers, `:tabs` lists tabs. This is why closing a window does not close the underlying file.

**Autocmds.** "When event X happens, run this Lua." The config uses them constantly:

```lua
vim.api.nvim_create_autocmd("LspAttach", {
  callback = function(ev) ... end,
})
```

Events you'll see here: `LspAttach` (LSP connects to a buffer), `FileType` (nvim detects a language), `BufWinEnter` (a buffer appears in a window), `VimLeavePre` (just before exit), `User PersistenceLoadPost` (custom event fired by persistence.nvim). Autocmds are the main way plugins and this config extend default behavior.

**Filetypes.** Nvim sniffs each buffer and sets `&filetype` (`lua`, `python`, `markdown`, etc.). Many features — syntax, LSP, our styling — key off this. Pi panels have custom filetypes like `pi-chat-history`, which is how we match them in `lua/core/options.lua` without hard-coding buffer names.

**Pack directories.** Neovim's built-in plugin loader. Anything under `~/.config/nvim/pack/plugins/start/<name>/` is added to `runtimepath` automatically on startup. No `packer.nvim`, no `lazy.nvim` — just clone a repo into that directory and it's installed. We deliberately chose this to keep the config simple and auditable.

**`require()`.** When your config calls `require("plugins.pi")`, Lua searches `runtimepath` for `lua/plugins/pi.lua` or `lua/plugins/pi/init.lua`. Both forms work. Our `lua/` tree mirrors the require paths: `require("core.options")` → `lua/core/options.lua`.

---

## File layout

```
init.lua                  entry point — order of requires
lua/
  core/
    options.lua           global vim options, pi-pane styling autocmds, window clamping
    keymaps.lua           non-LSP global keymaps (buffers, tabs, terminal)
  plugins/                one file per plugin (setup + keymaps together)
bin/
  pi-nvim                 wrapper that skips the buggy pi-pretty extension
```

Each plugin file calls its plugin's `.setup({...})` and registers any keymaps the plugin owns. Keymaps live next to the feature that provides them — git keymaps in the git files, LSP keymaps inside an `LspAttach` autocmd in `lsp.lua`. This is intentional: when you want to change a binding, you know where to look.

**Plugin pointer table:**

| File | Plugin | Provides |
|---|---|---|
| `colorscheme.lua` | tokyonight | theme |
| `ui.lua` | lualine, bufferline | statusline, tabline, tab rename |
| `fzf.lua` | fzf-lua | pickers (files, grep, buffers) |
| `completion.lua` | blink.cmp | autocomplete (manual trigger) |
| `lsp.lua` | mason, nvim-lspconfig | LSP installer + on-attach bindings |
| `gitsigns.lua` | gitsigns | gutter signs, inline blame, hunk ops |
| `diffview.lua` | diffview | diff viewer, PR review, history |
| `gitlinker.lua` | gitlinker | copy/open GitHub permalinks |
| `lazygit.lua` | — | floating lazygit + fzf git log |
| `trouble.lua` | trouble | diagnostics / quickfix panel |
| `grugfar.lua` | grug-far | search & replace across files |
| `whichkey.lua` | which-key | leader discovery popup |
| `session.lua` | persistence | per-cwd session save / restore |
| `alpha.lua` | alpha | side-by-side dashboard (image + session list) |
| `header.ansi` | — | chafa-generated colored image used by the dashboard |
| `render-markdown.lua` | render-markdown | markdown + pi chat rendering |
| `pi.lua` | alex35mil/pi.nvim | pi coding agent |

---

## Keymaps: where to look

Don't memorize a table here. The live source of truth is which-key.

- Press `<leader>` and pause — which-key pops a panel showing every leader binding, grouped by prefix (`b` buffer, `c` code, `g` git, `p` pi, `q` session, `<Tab>` tabs, and so on).
- Press `<leader>?` to see only the keymaps active for the *current buffer* (useful in LSP-attached files).
- Inside a specific plugin (e.g. lazygit's floating window or Mason's UI), press `g?` for that plugin's own keybindings.

A few non-obvious bindings worth memorizing because you'll use them constantly:

| Key | What |
|---|---|
| `<leader><leader>` | Find files (fzf) |
| `<leader>/` | Fuzzy search current buffer |
| `<leader>pc` | Toggle pi chat |
| `<leader>pC` | Continue last pi session |
| `<leader>qs` | Restore this directory's last session |
| `<leader>?` | Show keymaps for the current buffer |
| `<C-/>` | Open a terminal split |
| `gd`, `gr`, `K` | LSP: go-to-def, references, hover (in LSP'd buffers only) |

---

## Workflows

### Add a plugin

```bash
git clone --depth=1 <url> ~/.config/nvim/pack/plugins/start/<name>
```

Then write `lua/plugins/<name>.lua`:

```lua
require("<name>").setup({ ... })

vim.keymap.set("n", "<leader>xx", "<cmd>SomeCommand<cr>", { desc = "..." })
```

Add `require("plugins.<name>")` to `init.lua`. Restart nvim.

To make this reproducible on a fresh machine, append the git URL to the `plugins=()` array in `pde/lib/editor.sh` inside `install_editor()`.

### Add a keymap

Edit the plugin file where the feature lives (git stuff in `gitsigns.lua`/`diffview.lua`/etc.). Global non-LSP keymaps go in `lua/core/keymaps.lua`.

```lua
vim.keymap.set("n", "<leader>xx", "<cmd>SomeCommand<cr>", { desc = "What it does" })
```

The `desc` field is what which-key displays. Restart nvim to pick it up.

If the keymap logically belongs under a new group (e.g. `<leader>t` for "test"), add the group to `lua/plugins/whichkey.lua`'s `spec` so which-key labels it.

### Add an LSP server

Either:
1. `:Mason`, press `i` on the server you want. Installed for this session.
2. Add the server name to `ensure_installed` in `lua/plugins/lsp.lua` and restart. Installed automatically on next launch.

`mason-lspconfig` with `automatic_enable = true` attaches installed servers to matching filetypes without extra config. All the `<leader>c*` LSP keymaps in `lsp.lua`'s `LspAttach` autocmd automatically apply.

Common server names: `pyright`, `ts_ls` (TypeScript), `gopls`, `rust-analyzer`, `bashls`, `yamlls`, `jsonls`, `lua_ls`.

### Debug "nothing happened"

| Symptom | Check |
|---|---|
| LSP keymap missing | `:lua =vim.lsp.get_clients({ bufnr = 0 })` — empty means no LSP is attached to this filetype. Install a server. |
| Which-key group not labeled | Check `spec` in `lua/plugins/whichkey.lua`. Each group prefix needs a row. |
| Pi crashes on session reload | Verify `pi-nvim` is on `$PATH`: `which pi-nvim`. See "Pi setup" below. |
| Weird buffers after `<leader>qs` | See "Session restore" below — especially the `sessionoptions` tweaks. |
| Plugin not loading | Check `runtimepath`: `:lua =vim.opt.runtimepath:get()`. Verify the pack dir exists. |

---

## Session restore

`persistence.nvim` saves one session per `cwd` to `~/.local/state/nvim/sessions/`. `<leader>qs` loads the one for your current directory; `<leader>ql` loads the most recent across all directories.

What `lua/plugins/session.lua` customizes:

- **Single tab only.** `tabpages` removed from `sessionoptions` so restores don't resurrect forgotten tabs.
- **No empty buffers saved.** `blank` removed so `[No Name]` scratches stay out of the session file.
- **Globals persisted.** `globals` added to `sessionoptions` so `vim.g.SavedTabNames` (see below) survives.
- **Tab names preserved.** On `PersistenceSavePre`, we collect `vim.t.name` from each tab into `vim.g.SavedTabNames`; on `PersistenceLoadPost`, we reapply them. Nvim's default mksession doesn't save tab-local variables, so this is a manual bridge.
- **Scratch buffers wiped pre-load.** The alpha dashboard (an unnamed buffer) gets deleted before the session file is sourced, so it doesn't stack alongside restored windows.
- **Active-buffer sanity.** On save, if the active buffer is nameless, we switch to a real file first so mksession records something useful. On load, if we land on an empty buffer, we jump to the newest real file.
- **Diffview closed pre-save.** Diffview workspaces can't be serialized; we close them so the session file isn't polluted with dangling windows.
- **Pi auto-continue.** On `PersistenceLoadPost`, any zombie `pi-chat-*` buffers are wiped and `:PiContinue` runs, which reconnects pi and loads the last chat.

**Not restored:** diffview tabs. Reopen them manually after restoring (`<leader>gd`, `<leader>gD`, `<leader>gH`).

---

## Pi setup

Pi.nvim (alex35mil's version) spawns one `pi --mode rpc` subprocess per tab. We don't launch `pi` directly — we launch our wrapper `~/.local/bin/pi-nvim`, pointed at via the `bin = "pi-nvim"` option in `lua/plugins/pi.lua`.

**Why the wrapper.** The `@heyhuynhgiabuu/pi-pretty` extension captures a `ctx` reference inside a `setTimeout` callback. When pi's session is replaced (which happens any time you reload or continue a session), the captured ctx becomes stale and the timer fires into a crash. Upstream bug.

The wrapper avoids it without uninstalling pi-pretty globally. It reads `~/.pi/agent/settings.json`, and launches pi with `--no-extensions` plus explicit `-e <path>` for every configured extension *except* pi-pretty. Extensions added later via `pi install` are picked up automatically — no wrapper edit needed.

Source lives at `pde/config/nvim/bin/pi-nvim` and gets symlinked to `~/.local/bin/pi-nvim` by `install_editor()` in `pde/lib/editor.sh`.

---

## Dashboard (alpha)

Custom side-by-side layout: colored ASCII image on the left, menu + session list on the right, centered in the window (recomputes top padding on resize).

The image comes from `header.ansi`, a pre-rendered file produced by **chafa**. Neovim parses the ANSI escape sequences at startup, creates one highlight group per distinct color, and applies them per-row in alpha. chafa is only needed to *regenerate* the file.

### Regenerate the header image

```bash
chafa --format=symbols --symbols="ascii,-space" --size=45x22 --fg-only \
  /path/to/image.png > ~/.config/nvim/header.ansi
```

The `-space` option prevents blank cells (dark pixels become dots instead of spaces). Width/height scale both dimensions. If you change the image size, `lua/plugins/alpha.lua` auto-adapts — it measures rows at load time.

### Session list

The right column lists the 5 most recent persistence.nvim sessions (mtime-sorted). Pressing `1`–`5` `cd`s into that project and runs `:lua require('persistence').load()`. Core menu entries (`f`, `r`, `g`, `q`) stay above the sessions separator.

---

## Known quirks

- **Blink completion is manual-trigger.** The menu does not pop on every keystroke — press `<C-Space>` to open it. Exception: the pi prompt buffer has a dedicated pi source registered via `per_filetype`, still triggered manually.
- **LSP keymaps are buffer-local.** They only exist in buffers where a server has attached (via the `LspAttach` autocmd in `lsp.lua`). If you don't see `<leader>c*` in which-key, no LSP is attached to that filetype.
- **Pi panels use `winfixbuf`.** Clicking a bufferline tab from inside a pi window would normally error — bufferline's `left_mouse_command` override jumps to the first non-locked window first. Same wrapper protects `<S-h>` / `<S-l>` buffer cycling.
- **Which-key helix preset is heavy.** We've tuned it with `icons.mappings = false` and a `desc`-only filter. If it's still slow on your machine, change `preset` to `modern` or `classic` in `whichkey.lua`.
- **Tab labels auto-detect diffview and pi.** `ui.lua`'s `name_formatter` shows "Diff" for Diffview tabs and "Pi" for pi-chat tabs, otherwise uses `vim.t.name` (set via `<leader><Tab>r`) or the filename.
