local M = {}

local paths_env = vim.fn.expand("~/.config/pde/paths.env")

local function trim_quoted_env_value(value)
  if type(value) ~= "string" or #value < 2 then
    return value
  end

  local first = value:sub(1, 1)
  local last = value:sub(-1)
  if (first == '"' and last == '"') or (first == "'" and last == "'") then
    return value:sub(2, -2)
  end

  return value
end

function M.read(keys)
  local wanted = {}
  for _, key in ipairs(keys or {}) do
    wanted[key] = true
  end

  local file = io.open(paths_env, "r")
  if not file then
    return {}
  end

  local values = {}
  for line in file:lines() do
    local stripped = line:match("^%s*(.-)%s*$")
    if stripped ~= "" and stripped:sub(1, 1) ~= "#" then
      local key, value = stripped:match("^export%s+([A-Z0-9_]+)%s*=%s*(.-)%s*$")
      if not key then
        key, value = stripped:match("^([A-Z0-9_]+)%s*=%s*(.-)%s*$")
      end
      if key and wanted[key] then
        values[key] = trim_quoted_env_value(value)
      end
    end
  end
  file:close()

  return values
end

return M
