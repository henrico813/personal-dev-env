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

-- Configure vault roots locally so the tracked config stays portable.
local function add_workspace(name, env_name)
  local path = vim.env[env_name]
  if path and path ~= "" and vim.fn.isdirectory(path) == 1 then
    table.insert(workspaces, workspace(name, path))
  end
end

add_workspace("main", "VAULT")

if #workspaces == 0 then
  return
end

local ok, obsidian = pcall(require, "obsidian")
if not ok then
  return
end

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
  },
})

local map = function(lhs, rhs, desc)
  vim.keymap.set("n", lhs, rhs, { desc = desc, silent = true })
end

map("<leader>on", "<cmd>Obsidian new<cr>", "new note")
map("<leader>oo", "<cmd>Obsidian quick_switch<cr>", "open note")
map("<leader>ob", "<cmd>Obsidian backlinks<cr>", "backlinks")
map("<leader>ot", "<cmd>Obsidian tags<cr>", "tags")
map("<leader>os", "<cmd>Obsidian sync<cr>", "sync")
map("<leader>ol", "<cmd>Obsidian links<cr>", "links")
