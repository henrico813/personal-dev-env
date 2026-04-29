local main_vault = vim.env.PDE_MAIN_VAULT
local work_vault = vim.env.PDE_WORK_VAULT

local workspaces = {}
for _, w in ipairs({
  { name = "main", path = main_vault },
  { name = "work", path = work_vault },
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
  if work_vault and work_vault ~= "" then table.insert(dirs, work_vault) end
  if #dirs == 1 then
    fzf.files({ cwd = dirs[1] })
  elseif #dirs > 1 then
    fzf.files({ cwd = dirs[1], search_paths = dirs })
  end
end, "open note")

if main_vault and main_vault ~= "" then
  map("<leader>om", function() fzf.files({ cwd = main_vault }) end, "main vault")
end

if work_vault and work_vault ~= "" then
  map("<leader>ow", function() fzf.files({ cwd = work_vault }) end, "work vault")
end

map("<leader>os", function()
  local vaults = {}
  if main_vault and main_vault ~= "" then table.insert(vaults, main_vault) end
  if work_vault and work_vault ~= "" then table.insert(vaults, work_vault) end
  for _, path in ipairs(vaults) do
    vim.fn.jobstart({ "ob", "sync" }, { cwd = path })
  end
end, "sync vaults")
