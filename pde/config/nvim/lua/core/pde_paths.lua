local M = {}

local config_json = vim.fn.expand("~/.config/pde/config.json")

local function trim_quoted_env_value(value)
  return value
end

local function expand_env(value)
  return value
end

local function normalize_path_value(value)
  value = trim_quoted_env_value(value)
  if type(value) ~= "string" or value == "" then
    return value
  end

  value = expand_env(value)
  if value == "~" or value:match("^~/") then
    value = vim.fn.expand(value)
  elseif not value:match("^/") then
    value = vim.fn.fnamemodify(value, ":p")
  end

  return value
end

function M.read(keys, opts)
  local wanted = {}
  for _, key in ipairs(keys or {}) do
    wanted[key] = true
  end

  local path_keys = {}
  for _, key in ipairs(opts and opts.path_keys or {}) do
    path_keys[key] = true
  end

  local file = io.open(config_json, "r")
  if not file then
    return {}
  end

  local content = file:read("*a")
  file:close()

  local decoded_ok, data = pcall(vim.fn.json_decode, content)
  if not decoded_ok or type(data) ~= "table" then
    return {}
  end

  local raw = {
    PDE_INSTALL_PATH = data.install_path,
    PDE_PROFILE = data.profile,
    PDE_MAIN_VAULT = data.main_vault,
    PDE_WORK_VAULT = data.work_vault,
    PDE_DEFAULT_VAULT = data.default_vault,
    OPENCODE_BASE_URL = data.opencode_base_url,
    OPENCODE_INLINE_SHIM_PORT = data.opencode_inline_shim_port,
    OPENCODE_INLINE_MODEL = data.opencode_inline_model,
  }

  local values = {}
  for key, value in pairs(raw) do
    if wanted[key] then
      if path_keys[key] then
        values[key] = normalize_path_value(value)
      else
        values[key] = trim_quoted_env_value(value)
      end
    end
  end

  return values
end

return M
