-- CodeCompanion inline editing backed by OpenCode 2 through a local
-- OpenAI-compatible shim. CodeCompanion inline accepts only HTTP adapters,
-- so the ACP chat adapter cannot serve these requests directly.
local shim = require("opencode-inline.shim")

local M = {}

local defaults = {
  -- OpenCode 2 server; loopback URLs with an explicit port are auto-started.
  opencode_url = "http://127.0.0.1:4199",
  port = 4141,
  -- "provider/model[#variant]"; nil uses OpenCode's current default model.
  model = nil,
  -- Must name an installed OpenCode agent; "" uses OpenCode's default agent.
  agent = "inline",
  -- nil uses $OPENCODE_SERVER_PASSWORD, then a generated password stored in stdpath("state").
  password = nil,
  -- Shim executable; nil uses <plugin>/bin, then $PATH.
  cmd = nil,
}

M.config = vim.deepcopy(defaults)

local plugin_root = vim.fn.fnamemodify(debug.getinfo(1, "S").source:sub(2), ":p:h:h:h")
local transport_model = "opencode-inline"
local spinner_frames = { "-", "\\", "|", "/" }
local model_override
local model_chat
local feedback = { pending = 0, frame = 1, timer = nil, text = nil }

local function shim_bin()
  if M.config.cmd then
    return vim.fn.executable(M.config.cmd) == 1 and M.config.cmd or nil
  end
  local bundled = plugin_root .. "/bin/opencode-inline-shim"
  if vim.fn.executable(bundled) == 1 then
    return bundled
  end
  local bin = vim.fn.exepath("opencode-inline-shim")
  return bin ~= "" and bin or nil
end

local function notify_missing_bin()
  vim.notify("opencode-inline-shim not found; run `make build` in " .. plugin_root .. " or set cmd", vim.log.levels.WARN)
end

-- The shim passes this password to the OpenCode server it starts, so a stored
-- password keeps a detached server usable after the shim restarts.
local function server_password()
  if M.config.password and M.config.password ~= "" then
    return M.config.password
  end
  if vim.env.OPENCODE_SERVER_PASSWORD and vim.env.OPENCODE_SERVER_PASSWORD ~= "" then
    return vim.env.OPENCODE_SERVER_PASSWORD
  end

  local dir = vim.fn.stdpath("state") .. "/opencode-inline"
  local path = dir .. "/server-password"
  local file = io.open(path, "r")
  if file then
    local stored = vim.trim(file:read("*a") or "")
    file:close()
    if stored ~= "" then
      return stored
    end
  end

  local password = vim.uv.random(24):gsub(".", function(byte)
    return string.format("%02x", byte:byte())
  end)
  vim.fn.mkdir(dir, "p", tonumber("700", 8))
  file = assert(io.open(path, "w"))
  file:write(password, "\n")
  file:close()
  vim.uv.fs_chmod(path, tonumber("600", 8))
  return password
end

local function shim_spec()
  local bin = shim_bin()
  if not bin then
    return nil
  end
  local port = tostring(M.config.port)
  return {
    bin = bin,
    port = port,
    args = { "--port", port, "--opencode-url", M.config.opencode_url, "--agent", M.config.agent },
    env = { OPENCODE_SERVER_PASSWORD = server_password() },
  }
end

local function request_model()
  return model_override or M.config.model or transport_model
end

---CodeCompanion HTTP adapter for `adapters.http`.
function M.adapter()
  return require("codecompanion.adapters").extend("openai_compatible", {
    env = {
      api_key = "EMPTY",
      url = function()
        return "http://127.0.0.1:" .. tostring(M.config.port)
      end,
      chat_url = "/v1/chat/completions",
      models_endpoint = "/v1/models",
    },
    schema = {
      model = {
        default = request_model,
        choices = function()
          return { request_model() }
        end,
      },
    },
  })
end

---Start the shim, or reuse a healthy one.
---@param on_ready? fun(ready: boolean)
function M.start(on_ready)
  local spec = shim_spec()
  if not spec then
    notify_missing_bin()
    if on_ready then
      on_ready(false)
    end
    return
  end
  shim.start(spec, on_ready)
end

---Replace any running shim, e.g. after changing options or OpenCode servers.
function M.restart()
  local spec = shim_spec()
  if not spec then
    return notify_missing_bin()
  end
  shim.stop(spec)
  shim.start(spec)
end

---Prompt for an instruction and run CodeCompanion inline on the buffer or visual range.
function M.prompt()
  if not shim_bin() then
    return notify_missing_bin()
  end

  local opts = {}
  if vim.api.nvim_get_mode().mode:find("[vV\22]") then
    opts.range = 1
  end

  M.start()

  vim.ui.input({ prompt = require("codecompanion.config").display.action_palette.prompt }, function(input)
    if #vim.trim(input or "") == 0 then
      return
    end
    opts.args = input
    require("codecompanion").inline(opts)
  end)
end

local function default_model(chat)
  if M.config.model then
    return M.config.model
  end
  local models = chat.acp_connection and chat.acp_connection:get_models()
  return models and models.currentModelId or nil
end

-- The picker reads OpenCode's model list from a hidden CodeCompanion ACP chat.
local function ensure_model_chat(callback)
  model_chat = model_chat or require("codecompanion").chat({
    params = { adapter = "opencode" },
    auto_submit = false,
  })
  if not model_chat then
    return vim.notify("Failed to initialize OpenCode inline selector", vim.log.levels.ERROR)
  end

  if model_chat.acp_connection and model_chat.acp_connection:is_ready() then
    return callback(model_chat)
  end

  require("codecompanion.interactions.chat.helpers").create_acp_connection(model_chat, function()
    if model_chat and model_chat.acp_connection and model_chat.acp_connection:is_ready() then
      return callback(model_chat)
    end
    vim.notify("OpenCode inline selector is not ready", vim.log.levels.ERROR)
  end)
end

-- Adapts CodeCompanion's chat model picker so a choice sets the inline
-- override instead of changing the hidden chat's model.
local function selector_target(chat)
  return {
    adapter = { type = "acp" },
    acp_connection = {
      get_models = function()
        local models = chat.acp_connection and chat.acp_connection:get_models()
        if not models then
          return nil
        end
        models = vim.deepcopy(models)
        models.currentModelId = model_override or default_model(chat) or models.currentModelId
        return models
      end,
    },
    change_model = function(_, args)
      local model = args and args.model
      if not model or vim.trim(model) == "" then
        return
      end
      local default = default_model(chat)
      model_override = model ~= default and model or nil
      vim.notify("Inline model: " .. (model_override or default or model), vim.log.levels.INFO)
    end,
  }
end

---Pick the inline model for this Neovim session from OpenCode's ACP model list.
function M.select_model()
  ensure_model_chat(function(chat)
    require("codecompanion.interactions.chat.keymaps.change_adapter").select_model(selector_target(chat))
  end)
end

---Spinner text while inline requests are pending, or "".
function M.status()
  return feedback.text or ""
end

local function set_status(text)
  feedback.text = text
  vim.schedule(function()
    vim.cmd("redrawstatus")
  end)
end

local function start_feedback()
  feedback.pending = feedback.pending + 1
  if feedback.pending > 1 then
    return
  end

  feedback.frame = 1
  set_status(spinner_frames[feedback.frame] .. " Inline")
  feedback.timer = vim.uv.new_timer()
  feedback.timer:start(120, 120, function()
    feedback.frame = (feedback.frame % #spinner_frames) + 1
    set_status(spinner_frames[feedback.frame] .. " Inline")
  end)

  vim.notify("Generating inline edit...", vim.log.levels.INFO)
end

local function stop_feedback(failed)
  if feedback.pending > 0 then
    feedback.pending = feedback.pending - 1
  end
  if failed then
    vim.notify("Inline edit failed", vim.log.levels.WARN)
  end
  if feedback.pending > 0 then
    return
  end

  if feedback.timer then
    feedback.timer:stop()
    feedback.timer:close()
    feedback.timer = nil
  end
  set_status(nil)
end

local function is_inline_request(args)
  local data = args and args.data
  if type(data) ~= "table" then
    return false
  end
  return data.kind == "inline" or data.type == "inline" or data.interaction == "inline"
end

---@param opts? table Overrides for the defaults above.
function M.setup(opts)
  M.config = vim.tbl_deep_extend("force", vim.deepcopy(defaults), opts or {})

  local group = vim.api.nvim_create_augroup("OpenCodeInlineFeedback", { clear = true })
  vim.api.nvim_create_autocmd("User", {
    group = group,
    pattern = "CodeCompanionRequestStarted",
    callback = function(args)
      if is_inline_request(args) then
        start_feedback()
      end
    end,
  })
  vim.api.nvim_create_autocmd("User", {
    group = group,
    pattern = "CodeCompanionRequestFinished",
    callback = function(args)
      if is_inline_request(args) then
        stop_feedback(vim.tbl_get(args, "data", "status") ~= "success")
      end
    end,
  })

  vim.api.nvim_create_user_command("OpenCodeInlineRestart", M.restart, { desc = "Restart the OpenCode inline shim" })
end

return M
