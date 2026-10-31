#!/usr/bin/env bash
# Resizes headless Neovim to phone width and back, and checks that windows,
# the status and tab lines, and Markdown folds follow the screen width.
# Usage: pde-installer/test/nvim-phone-view.sh
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
printf 'local x = 1\n' >"$tmp/code.lua"
printf 'notes\n' >"$tmp/notes.txt"
printf '# Desktop\n' >"$tmp/desktop.md"
# A plan: frontmatter, a diff block with its header, and a ```` block that
# shows a ``` block inside it.
cat >"$tmp/plan.md" <<'EOF_MD'
---
status: open
---
# Plan

Some text.

```diff
diff --git a/lua/x.lua b/lua/x.lua
--- a/lua/x.lua
+++ b/lua/x.lua
@@ -1 +1 @@
-local x = 1
+local x = 2
```

More text.

````markdown
```lua
y()
```
````

End.
EOF_MD

cat >"$tmp/driver.lua" <<'EOF_LUA'
local config = vim.env.NVIM_CONFIG
local dir = vim.env.FILES
vim.opt.rtp:prepend(config)
vim.opt.rtp:append(config .. "/after")
dofile(config .. "/lua/core/options.lua")

-- Acts like bufferline, which shows its tab line whenever a buffer is added.
vim.api.nvim_create_autocmd("BufAdd", {
  callback = function()
    vim.o.showtabline = 2
  end,
})

require("core.phone_view").setup()

-- Headless Neovim does not send VimResized itself.
local function resize(columns)
  vim.o.columns = columns
  vim.api.nvim_exec_autocmds("VimResized", {})
end

-- Returns the first and last line of the closed fold at lnum, or -1.
local function closed_fold(win, lnum)
  local fold = vim.api.nvim_win_call(win, function()
    return { vim.fn.foldclosed(lnum), vim.fn.foldclosedend(lnum) }
  end)
  return fold[1], fold[2]
end

-- On the desktop, Markdown keeps the wrapping its filetype sets. The width
-- is set because headless Neovim starts at 80 columns.
resize(120)
vim.cmd.edit(dir .. "/desktop.md")

assert(vim.wo.wrap, "a Markdown file opened on the desktop must keep wrapping")

-- A code window, and a window with its own values, as CodeCompanion sets.
vim.cmd.edit(dir .. "/code.lua")
local code_win = vim.api.nvim_get_current_win()
local code_buf = vim.api.nvim_get_current_buf()
vim.cmd("vsplit | enew")
local chat_win = vim.api.nvim_get_current_win()
vim.wo[0][0].number = false
vim.wo[0][0].wrap = true

-- On the phone, open the plan in the code window, leave it and come back,
-- then open notes in a new window.
resize(40)
vim.api.nvim_set_current_win(code_win)
vim.cmd.edit(dir .. "/plan.md")
local plan_buf = vim.api.nvim_get_current_buf()
vim.cmd.buffer(code_buf)
vim.cmd.buffer(plan_buf)
vim.cmd.new(dir .. "/notes.txt")
local notes_win = vim.api.nvim_get_current_win()

for _, win in ipairs({ code_win, notes_win }) do
  assert(not vim.wo[win].number, "phone view must hide line numbers")
  assert(vim.wo[win].signcolumn == "no", "phone view must hide signs")
  assert(vim.wo[win].wrap, "phone view must wrap long lines")
end
assert(vim.o.laststatus == 0, "phone view must hide the status line")
assert(vim.o.showtabline == 0, "a new buffer must not bring the tab line back")

-- Each code block and the frontmatter fold to one line; text stays open.
local folds = {
  { 1, 1, 3 },
  { 6, -1, -1 },
  { 8, 8, 15 },
  { 17, -1, -1 },
  { 19, 19, 23 },
  { 25, -1, -1 },
}
for _, fold in ipairs(folds) do
  local first, last = closed_fold(code_win, fold[1])
  assert(first == fold[2] and last == fold[3], "wrong fold at line " .. fold[1])
end
local label = vim.api.nvim_win_call(code_win, function()
  return vim.fn.foldtextresult(8)
end)
assert(label == "▸ x.lua · 8 lines", "a diff fold must name its file: " .. label)

-- A block typed in while reading gets its own fold, and the text after it
-- stays open.
vim.api.nvim_buf_set_lines(plan_buf, 17, 17, false, { "", "```sh", "echo", "```" })

assert(closed_fold(code_win, 19) == 19, "a new block must fold")
assert(closed_fold(code_win, 23) == 23, "a later block must still fold")
assert(closed_fold(code_win, 29) == -1, "text after a new block must not fold")

-- zj from an opened diff block goes to the next block, past its header fold.
local next_block = vim.api.nvim_win_call(code_win, function()
  vim.api.nvim_win_set_cursor(0, { 8, 0 })
  vim.cmd("normal! zo")
  vim.cmd("normal zj")
  return vim.fn.line(".")
end)

assert(next_block == 19, "zj must reach the next block, not a diff header")

-- With the cursor in the open diff block and the others closed, sa opens
-- them all, and a second sa closes them all.
vim.api.nvim_win_call(code_win, function()
  vim.api.nvim_win_set_cursor(0, { 13, 0 })
  vim.cmd("normal sa")
end)

assert(closed_fold(code_win, 1) == -1, "sa must open every block")

vim.api.nvim_win_call(code_win, function()
  vim.cmd("normal sa")
end)

assert(closed_fold(code_win, 8) == 8, "a second sa must close every block")

-- Open every block with zR, which changes foldlevel, and leave the cursor
-- in the diff block.
vim.api.nvim_win_call(code_win, function()
  vim.cmd("normal! zR")
  vim.api.nvim_win_set_cursor(0, { 13, 0 })
end)

-- Back on the desktop.
resize(120)

assert(vim.wo[code_win].number, "the plan's window must get numbers back")
assert(vim.wo[code_win].wrap, "a plan opened on the phone must keep wrapping")
assert(closed_fold(code_win, 8) == -1, "the desktop must not keep phone folds")
assert(not vim.wo[chat_win].number, "a window's own values must survive")
assert(vim.wo[chat_win].wrap, "a window's own values must survive")
assert(not vim.wo[notes_win].wrap, "a new window must get desktop values")
assert(vim.o.laststatus == 2, "the status line must come back")
assert(vim.o.showtabline == 2, "the tab line must come back")

-- Turning the phone back keeps the block being read open.
resize(40)

assert(closed_fold(code_win, 13) == -1, "the block being read must stay open")
assert(closed_fold(code_win, 23) == 23, "other blocks must fold again")

-- A buffer hidden on the phone comes back with the options it last had.
resize(120)
vim.api.nvim_set_current_win(code_win)
vim.cmd.buffer(code_buf)

assert(vim.wo[code_win].number, "a hidden buffer must not keep phone values")
assert(not vim.wo[code_win].wrap, "a hidden buffer must not keep phone values")
EOF_LUA
NVIM_CONFIG="$REPO_ROOT/chezmoi/dot_config/nvim" FILES="$tmp" \
	nvim --clean -l "$tmp/driver.lua"

echo "nvim phone view: ok"
