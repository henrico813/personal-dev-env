local headless_dir = vim.fn.expand("~/.local/share/pde/obsidian-headless")

local function workspace(name, path)
  return {
    name = name,
    path = path,
    overrides = {
      daily_notes = {
        folder = "100 Work Notes/101 Daily Notes",
      },
      notes_subdir = "800 Staging",
      new_notes_location = "notes_subdir",
    },
  }
end

local workspaces = {}
for _, ws in ipairs({
  workspace("main", "/mnt/nas_hco/Main Vault"),
  workspace("work", "/mnt/nas_hco/Work Notes"),
}) do
  if vim.fn.isdirectory(ws.path) == 1 then
    table.insert(workspaces, ws)
  end
end

if #workspaces == 0 then
  return
end

local obsidian = require("obsidian")

obsidian.setup({
  workspaces = workspaces,
  picker = {
    name = "fzf-lua",
  },
  ui = {
    enable = false,
  },
  sync = {
    enabled = true,
    device_name = vim.fn.hostname(),
    binary = headless_dir .. "/bin/ob",
  },
})

local map = function(lhs, rhs, desc)
  vim.keymap.set("n", lhs, rhs, { desc = desc, silent = true })
end

map("<leader>on", "<cmd>Obsidian new<cr>", "new note")
map("<leader>oo", "<cmd>Obsidian quick_switch<cr>", "open note")
map("<leader>od", "<cmd>Obsidian today<cr>", "daily note")
map("<leader>ob", "<cmd>Obsidian backlinks<cr>", "backlinks")
map("<leader>ot", "<cmd>Obsidian tags<cr>", "tags")
map("<leader>os", "<cmd>Obsidian sync<cr>", "sync")
map("<leader>ol", "<cmd>Obsidian links<cr>", "links")
