local M = {}

local main_vault = vim.env.PDE_MAIN_VAULT
local work_vault = vim.env.PDE_WORK_VAULT

local vaults = {
  main = { name = "main", path = main_vault, env = "PDE_MAIN_VAULT" },
  work = { name = "work", path = work_vault, env = "PDE_WORK_VAULT" },
}

local sync_states = {
  main = {
    running = false,
    job_id = nil,
    last_ok_at = nil,
    last_error = nil,
    last_message = nil,
  },
  work = {
    running = false,
    job_id = nil,
    last_ok_at = nil,
    last_error = nil,
    last_message = nil,
  },
}

local cached_sync_status = ""

local function is_configured(vault)
  return vault.path and vault.path ~= "" and vim.fn.isdirectory(vault.path) == 1
end

local function configured_vaults()
  local out = {}
  for _, key in ipairs({ "main", "work" }) do
    local vault = vaults[key]
    if is_configured(vault) then
      table.insert(out, { name = vault.name, path = vault.path })
    end
  end
  return out
end

local workspaces = configured_vaults()

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
  if is_configured(vaults.main) then table.insert(dirs, vaults.main.path) end
  if is_configured(vaults.work) then table.insert(dirs, vaults.work.path) end
  if #dirs == 0 then
    vim.notify("No vaults configured. Set PDE_MAIN_VAULT or PDE_WORK_VAULT in ~/.config/pde/paths.env", vim.log.levels.WARN)
  elseif #dirs == 1 then
    fzf.files({ cwd = dirs[1] })
  else
    fzf.files({ cwd = dirs[1], search_paths = dirs })
  end
end, "open note")

map("<leader>om", function() vault_files(vaults.main.path, vaults.main.env) end, "main vault")
map("<leader>ow", function() vault_files(vaults.work.path, vaults.work.env) end, "work vault")

local function format_time(ts)
  return ts and os.date("%H:%M", ts) or nil
end

local function rebuild_sync_status()
  local parts = {}
  for _, key in ipairs({ "main", "work" }) do
    local state = sync_states[key]
    local label = vaults[key].name
    if state.running then
      table.insert(parts, "󰓦 " .. label .. " syncing")
    elseif state.last_error then
      table.insert(parts, "󰅚 " .. label .. " failed")
    elseif state.last_ok_at then
      table.insert(parts, "󰄬 " .. label .. " " .. format_time(state.last_ok_at))
    end
  end
  cached_sync_status = table.concat(parts, " · ")
end

local function refresh_sync_status()
  rebuild_sync_status()
  vim.schedule(function()
    vim.cmd("redrawstatus")
  end)
end

function M.sync_status()
  return cached_sync_status
end

local function first_nonempty(data)
  if not data then return nil end
  for _, line in ipairs(data) do
    if line and line ~= "" then
      return line
    end
  end
  return nil
end

local function sync_vault(key)
  local vault = vaults[key]
  local state = sync_states[key]
  if not is_configured(vault) then
    vim.notify(unset_msg(vault.env), vim.log.levels.WARN)
    return
  end
  if state.running then
    vim.notify("Sync already running: " .. vault.name, vim.log.levels.INFO)
    return
  end

  local stderr_buf = {}
  state.running = true
  state.last_error = nil
  state.last_message = "syncing"
  refresh_sync_status()
  vim.notify("Syncing " .. vault.name .. "...", vim.log.levels.INFO)

  local job_id = vim.fn.jobstart({ "ob", "sync" }, {
    cwd = vault.path,
    on_stdout = function(_, data)
      local line = first_nonempty(data)
      if line then
        state.last_message = line
      end
    end,
    on_stderr = function(_, data)
      local line = first_nonempty(data)
      if line then
        table.insert(stderr_buf, line)
        state.last_message = line
      end
    end,
    on_exit = function(_, code)
      state.running = false
      state.job_id = nil
      if code == 0 then
        state.last_ok_at = os.time()
        state.last_error = nil
        state.last_message = "synced"
      else
        state.last_error = stderr_buf[1] or ("exit " .. code)
        state.last_message = state.last_error
      end
      vim.schedule(function()
        refresh_sync_status()
        if code == 0 then
          vim.notify("Sync complete: " .. vault.name, vim.log.levels.INFO)
        else
          vim.notify("Sync failed: " .. vault.name .. " - " .. state.last_error, vim.log.levels.WARN)
        end
      end)
    end,
  })

  state.job_id = job_id > 0 and job_id or nil
  if not state.job_id then
    state.running = false
    state.last_error = "failed to start sync job"
    state.last_message = state.last_error
    refresh_sync_status()
    vim.notify("Sync failed: " .. vault.name .. " - " .. state.last_error, vim.log.levels.WARN)
  end
end

map("<leader>os", function()
  local any = false
  for _, key in ipairs({ "main", "work" }) do
    if is_configured(vaults[key]) then
      any = true
      sync_vault(key)
    end
  end
  if not any then
    vim.notify("No vaults configured. Set PDE_MAIN_VAULT or PDE_WORK_VAULT in ~/.config/pde/paths.env", vim.log.levels.WARN)
  end
end, "sync vaults")

return M
