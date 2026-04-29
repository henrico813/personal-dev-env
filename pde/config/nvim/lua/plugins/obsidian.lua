local M = {}

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
if ok and #workspaces > 0 then
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

local function unset_msg(label)
  return label .. " not configured. Set " .. label .. " in ~/.config/pde/paths.env"
end

local function vault_files(path, label)
  if not path or path == "" then
    vim.notify(unset_msg(label), vim.log.levels.WARN)
    return
  end
  fzf.files({ cwd = path })
end

map("<leader>oo", function()
  local dirs = {}
  if main_vault and main_vault ~= "" then table.insert(dirs, main_vault) end
  if extra and extra ~= "" then table.insert(dirs, extra) end
  if #dirs == 0 then
    vim.notify("No vaults configured. Set PDE_MAIN_VAULT or PDE_extra in ~/.config/pde/paths.env", vim.log.levels.WARN)
  elseif #dirs == 1 then
    fzf.files({ cwd = dirs[1] })
  else
    fzf.files({ cwd = dirs[1], search_paths = dirs })
  end
end, "open note")

map("<leader>om", function() vault_files(main_vault, "PDE_MAIN_VAULT") end, "main vault")
map("<leader>ow", function() vault_files(extra, "PDE_extra") end, "extra")

local active_syncs = {}

function M.sync_status()
  local names = {}
  for path in pairs(active_syncs) do
    table.insert(names, vim.fn.fnamemodify(path, ":t"))
  end
  if #names == 0 then return "" end
  table.sort(names)
  return "󰓦 syncing " .. table.concat(names, ", ")
end

local function sync_vault(path)
  local name = vim.fn.fnamemodify(path, ":t")
  if active_syncs[path] then
    vim.notify("Sync already running: " .. name, vim.log.levels.INFO)
    return
  end

  local stderr_buf = {}
  vim.notify("Syncing " .. name .. "...", vim.log.levels.INFO)
  active_syncs[path] = vim.fn.jobstart({ "ob", "sync" }, {
    cwd = path,
    on_stderr = function(_, data)
      if data then
        for _, line in ipairs(data) do
          if line ~= "" then table.insert(stderr_buf, line) end
        end
      end
    end,
    on_exit = function(_, code)
      active_syncs[path] = nil
      vim.schedule(function()
        vim.cmd("redrawstatus")
        if code == 0 then
          vim.notify("Sync complete: " .. name, vim.log.levels.INFO)
        else
          local err = stderr_buf[1] or ("exit " .. code)
          vim.notify("Sync failed: " .. name .. " - " .. err, vim.log.levels.WARN)
        end
      end)
    end,
  })
  vim.cmd("redrawstatus")
end

map("<leader>os", function()
  local vaults = {}
  if main_vault and main_vault ~= "" then table.insert(vaults, main_vault) end
  if extra and extra ~= "" then table.insert(vaults, extra) end
  if #vaults == 0 then
    vim.notify("No vaults configured. Set PDE_MAIN_VAULT or PDE_extra in ~/.config/pde/paths.env", vim.log.levels.WARN)
    return
  end
  for _, path in ipairs(vaults) do
    sync_vault(path)
  end
end, "sync vaults")

return M
