local M = {}

local pde_paths = require("core.pde_paths")

-- The same lookup nobs uses: $XDG_CONFIG_HOME/nobs/config.json when
-- XDG_CONFIG_HOME is absolute, otherwise ~/.config/nobs/config.json.
local function nobs_settings_path()
  local xdg = vim.env.XDG_CONFIG_HOME
  if xdg and xdg:sub(1, 1) == "/" then
    return xdg .. "/nobs/config.json"
  end
  return vim.fn.expand("~/.config/nobs/config.json")
end

local settings_path = nobs_settings_path()

-- Vaults added with `nobs vault set`, sorted by name. No file means no vaults.
local function read_vaults()
  local file = io.open(settings_path, "r")
  if not file then
    return {}
  end

  local content = file:read("*a")
  file:close()

  local ok, data = pcall(vim.json.decode, content)
  if not ok or type(data) ~= "table" then
    vim.notify_once("Invalid " .. settings_path, vim.log.levels.WARN)
    return {}
  end
  -- nobs reads a missing or null vaults list as no vaults.
  if data.vaults == nil or data.vaults == vim.NIL then
    return {}
  end
  if type(data.vaults) ~= "table" then
    vim.notify_once("Invalid " .. settings_path, vim.log.levels.WARN)
    return {}
  end

  local out = {}
  for name, vault in pairs(data.vaults) do
    if type(vault) == "table" and type(vault.path) == "string" then
      table.insert(out, { name = name, path = vault.path })
    end
  end
  table.sort(out, function(a, b) return a.name < b.name end)
  return out
end

local vaults = read_vaults()

-- Until `nobs vault set` creates the nobs settings file, the vaults in
-- ~/.config/pde/config.json are not used. Nothing is saved, so the warning
-- stops once the file exists.
if vim.fn.filereadable(settings_path) == 0 then
  local old = pde_paths.read({ "PDE_MAIN_VAULT", "PDE_WORK_VAULT" })
  if (old.PDE_MAIN_VAULT or "") ~= "" or (old.PDE_WORK_VAULT or "") ~= "" then
    vim.notify_once(
      "Neovim no longer reads vaults from ~/.config/pde/config.json. Run nobs vault set NAME PATH for each one, then restart Neovim.",
      vim.log.levels.WARN
    )
  end
end

local workspaces = {}
for _, vault in ipairs(vaults) do
  if vim.fn.isdirectory(vault.path) == 1 then
    table.insert(workspaces, vault)
  end
end

local ok, obsidian = pcall(require, "obsidian")
if ok and #workspaces > 0 then
  obsidian.setup({
    workspaces = workspaces,
    legacy_commands = false,
    ui = { enable = false },
    statusline = { enabled = false },
    footer = { enabled = false },
  })
end

local map = function(lhs, rhs, desc)
  vim.keymap.set("n", lhs, rhs, { desc = desc, silent = true })
end

local fzf = require("fzf-lua")

local no_vaults = "No vault folders found. Run nobs vault set NAME PATH, then restart Neovim"

-- Lists notes even when the vault's .gitignore skips them all, but leaves out
-- dot folders such as .obsidian and .trash. Pass no_ignore = true with it, so
-- fzf-lua's Alt-i hides ignored notes on the first press.
local notes_cmd = "rg --files --no-ignore --follow --glob '!.git'"

map("<leader>oo", function()
  local dirs = {}
  for _, vault in ipairs(workspaces) do
    table.insert(dirs, vault.path)
  end
  if #dirs == 0 then
    vim.notify(no_vaults, vim.log.levels.WARN)
  elseif #dirs == 1 then
    fzf.files({ cwd = dirs[1], cmd = notes_cmd, no_ignore = true })
  else
    -- fzf-lua ignores search_paths when cmd is set, so the paths go in cmd.
    local paths = vim.tbl_map(vim.fn.shellescape, dirs)
    fzf.files({
      cwd = dirs[1],
      cmd = notes_cmd .. " " .. table.concat(paths, " "),
      no_ignore = true,
    })
  end
end, "open note")

local function vault_files(name)
  for _, vault in ipairs(workspaces) do
    if vault.name == name then
      fzf.files({ cwd = vault.path, cmd = notes_cmd, no_ignore = true })
      return
    end
  end
  vim.notify("No vault named " .. name, vim.log.levels.WARN)
end

map("<leader>om", function() vault_files("main") end, "main vault")
map("<leader>ow", function() vault_files("work") end, "work vault")

-- Neovim stops nobs after 2 minutes. It cannot stop a docker
-- compose that nobs started, so the result shows when that ends. The broker's
-- sync runs on to its own limit, and nothing is left locked.
M.sync_timeout_ms = 120000

local function last_line(text)
  local line
  for each in (text or ""):gmatch("[^\n]+") do
    if vim.trim(each) ~= "" then
      line = each
    end
  end
  return line
end

local function sync_vault(vault)
  local timeout = M.sync_timeout_ms
  local function report(result)
    local message, level = "Sync complete: " .. vault.name, vim.log.levels.INFO
    -- Only the exit status means failure, never stderr: starting a stopped
    -- broker prints Docker's progress there, and nobs prints its error last.
    -- A process killed by a signal reports code 0, so check the signal too.
    if result.code ~= 0 or result.signal ~= 0 then
      local fallback = result.signal ~= 0 and ("signal " .. result.signal) or ("exit " .. result.code)
      local reason = last_line(result.stderr) or fallback
      if result.code == 124 then
        -- vim.system's exit code when it stops the command at its time limit.
        reason = "timed out after " .. timeout / 1000 .. "s"
      end
      message, level = "Sync failed: " .. vault.name .. " - " .. reason, vim.log.levels.WARN
    end
    vim.schedule(function() vim.notify(message, level) end)
  end

  vim.notify("Syncing " .. vault.name .. "...", vim.log.levels.INFO)
  local started, err = pcall(vim.system, { "nobs", "--vault", vault.name, "sync-now" },
    { stdout = false, text = true, timeout = timeout }, report)
  if not started then
    local reason = tostring(err):find("ENOENT") and "nobs not found on PATH" or tostring(err)
    report({ code = -1, signal = 0, stderr = reason })
  end
end

map("<leader>os", function()
  if #vaults == 0 then
    vim.notify("No vaults set up. Run nobs vault set NAME PATH, then restart Neovim", vim.log.levels.WARN)
  end
  for _, vault in ipairs(vaults) do
    sync_vault(vault)
  end
end, "sync vaults")

return M
