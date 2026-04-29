local main_vault = vim.env.PDE_MAIN_VAULT
local extra = vim.env.PDE_extra

local workspaces = {}
for _, w in ipairs({
  { name = "main", path = main_vault },
  { name = "work", path = extra },
}) do
  if w.path and w.path ~= "" and vim.fn.isdirectory(w.path) == 1 then
    table.insert(workspaces, w)
  end
end

local ok, obsidian = pcall(require, "obsidian")
if not ok then
  return
end

if #workspaces > 0 then
  obsidian.setup({
    workspaces = workspaces,
    legacy_commands = false,
    ui = { enable = false },
  })
end

local map = function(lhs, rhs, desc)
  vim.keymap.set("n", lhs, rhs, { desc = desc, silent = true })
end

local fzf = require("fzf-lua")

map("<leader>oo", function()
  local dirs = {}
  if main_vault and main_vault ~= "" then table.insert(dirs, main_vault) end
  if extra and extra ~= "" then table.insert(dirs, extra) end
  if #dirs == 1 then
    fzf.files({ cwd = dirs[1] })
  elseif #dirs > 1 then
    fzf.files({ cwd = dirs[1], search_paths = dirs })
  end
end, "open note")

if main_vault and main_vault ~= "" then
  map("<leader>om", function() fzf.files({ cwd = main_vault }) end, "main vault")
end

if extra and extra ~= "" then
  map("<leader>ow", function() fzf.files({ cwd = extra }) end, "extra")
end

map("<leader>os", function()
  local vaults = {}
  if main_vault and main_vault ~= "" then table.insert(vaults, main_vault) end
  if extra and extra ~= "" then table.insert(vaults, extra) end
  for _, path in ipairs(vaults) do
    vim.fn.jobstart({ "ob", "sync" }, { cwd = path })
  end
end, "sync vaults")
