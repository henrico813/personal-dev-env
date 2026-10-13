local M = {}

local config_json = vim.fn.expand("~/.config/pde/config.json")

function M.read(keys)
  local wanted = {}
  for _, key in ipairs(keys or {}) do
    wanted[key] = true
  end

  local file = io.open(config_json, "r")
  if not file then
    return {}
  end

  local content = file:read("*a")
  file:close()

  local decoded_ok, data = pcall(vim.fn.json_decode, content)
  if not decoded_ok or type(data) ~= "table" then
    vim.notify_once("Invalid ~/.config/pde/config.json", vim.log.levels.WARN)
    return {}
  end

  -- The vault keys remain only so Neovim can warn that they are no longer used.
  local raw = {
    PDE_INSTALL_PATH = data.install_path,
    PDE_MAIN_VAULT = data.main_vault,
    PDE_WORK_VAULT = data.work_vault,
    OPENCODE_BASE_URL = data.opencode_base_url,
    OPENCODE_INLINE_SHIM_PORT = data.opencode_inline_shim_port,
    OPENCODE_INLINE_MODEL = data.opencode_inline_model,
  }

  local values = {}
  for key, value in pairs(raw) do
    if wanted[key] then
      values[key] = value
    end
  end

  return values
end

return M
