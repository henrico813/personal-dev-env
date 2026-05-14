local shim_job
local inline_model_chat
local spinner_frames = { "-", "\\", "|", "/" }

local pde_paths = require("core.pde_paths")

local opencode_env_keys = { "OPENCODE_BASE_URL", "OPENCODE_INLINE_SHIM_PORT", "OPENCODE_INLINE_MODEL" }
local inherited_opencode_env = {}

for _, key in ipairs(opencode_env_keys) do
  inherited_opencode_env[key] = vim.env[key] ~= nil and vim.env[key] ~= ""
end

local function refresh_opencode_env()
  local values = pde_paths.read(opencode_env_keys)
  for _, key in ipairs(opencode_env_keys) do
    if not inherited_opencode_env[key] then
      if values[key] and values[key] ~= "" then
        vim.env[key] = values[key]
      else
        vim.env[key] = nil
      end
    end
  end
end

refresh_opencode_env()

local function shim_port()
  refresh_opencode_env()
  return vim.env.OPENCODE_INLINE_SHIM_PORT or "4141"
end

local function shim_url()
  return "http://127.0.0.1:" .. shim_port()
end

local function configured_inline_model()
  refresh_opencode_env()
  local model = vim.env.OPENCODE_INLINE_MODEL
  if not model or vim.trim(model) == "" or model == "opencode-inline" then
    return nil
  end
  return model
end

local function inline_request_model()
  return vim.g.pde_inline_model or configured_inline_model() or "opencode-inline"
end

local function shim_bin()
  local bin = vim.fn.exepath("opencode-inline-shim")
  if bin == "" then
    return nil
  end
  return bin
end

local shim_pidfile = vim.fn.stdpath("state") .. "/opencode-inline-shim.pid"

local function read_shim_pidfile()
  local file = io.open(shim_pidfile, "r")
  if not file then
    return nil
  end

  local contents = file:read("*a")
  file:close()
  return tonumber(vim.trim(contents or ""))
end

local function write_shim_pidfile(pid)
  if not pid or pid <= 0 then
    return
  end

  local file = io.open(shim_pidfile, "w")
  if not file then
    return
  end

  file:write(tostring(pid), "\n")
  file:close()
end

local function clear_shim_pidfile()
  pcall(vim.fn.delete, shim_pidfile)
end

local function process_command(pid)
  if not pid or pid <= 0 then
    return nil
  end

  local ok, result = pcall(function()
    return vim.system({ "ps", "-o", "command=", "-p", tostring(pid) }, { text = true }):wait()
  end)
  if not ok or not result or result.code ~= 0 or not result.stdout then
    return nil
  end

  local command = vim.trim(result.stdout)
  if command == "" then
    return nil
  end
  return command
end

local function inline_shim_pid(pid)
  local command = process_command(pid)
  if not command or not command:find("opencode%-inline%-shim", 1, false) then
    return nil
  end
  return pid
end

local function kill_inline_shim_pid(pid)
  pid = inline_shim_pid(pid)
  if not pid then
    return
  end

  pcall(function()
    vim.system({ "kill", tostring(pid) }):wait()
  end)
end

local function shim_port_listener_output()
  local ok, result = pcall(function()
    return vim.system({ "ss", "-ltnp", "( sport = :" .. shim_port() .. " )" }, { text = true }):wait()
  end)
  if not ok or not result or result.code ~= 0 or not result.stdout then
    return nil
  end

  return result.stdout
end

local function listener_exists_on_shim_port()
  local output = shim_port_listener_output()
  if not output then
    return false
  end

  local lines = 0
  for line in output:gmatch("[^\n]+") do
    if vim.trim(line) ~= "" then
      lines = lines + 1
    end
  end
  return lines > 1
end

local function listener_pid_on_shim_port()
  local output = shim_port_listener_output()
  if not output then
    return nil
  end

  return inline_shim_pid(tonumber(output:match("pid=(%d+)")))
end

local function cleanup_stale_inline_shim(opts)
  opts = opts or {}
  local tracked_pid = read_shim_pidfile()
  if opts.kill_tracked ~= false then
    kill_inline_shim_pid(tracked_pid)
  end

  local listener_pid = listener_pid_on_shim_port()
  if listener_pid and opts.kill_listener ~= false and listener_pid ~= tracked_pid then
    kill_inline_shim_pid(listener_pid)
  end

  if opts.wait_for_release then
    vim.wait(1500, function()
      return not listener_exists_on_shim_port()
    end, 50)
  end

  clear_shim_pidfile()
end

local function shim_healthcheck(callback)
  refresh_opencode_env()
  local bin = shim_bin()
  if not bin then
    return callback(false)
  end
  vim.system({ bin, "--healthcheck" }, { text = true }, function(result)
    vim.schedule(function()
      callback(result.code == 0)
    end)
  end)
end

local function wait_for_shim_ready(attempts, callback)
  shim_healthcheck(function(ready)
    if ready or attempts <= 1 then
      return callback(ready)
    end
    vim.defer_fn(function()
      wait_for_shim_ready(attempts - 1, callback)
    end, 150)
  end)
end

local function stop_inline_shim()
  if shim_job then
    pcall(vim.fn.jobstop, shim_job)
    shim_job = nil
  end
  cleanup_stale_inline_shim({ wait_for_release = true })
  clear_shim_pidfile()
end

local function tracked_shim_running()
  return shim_job and vim.fn.jobwait({ shim_job }, 0)[1] == -1
end

local function start_inline_shim(opts)
  opts = opts or {}
  refresh_opencode_env()
  local bin = shim_bin()
  if not bin then
    if opts.notify_missing then
      vim.notify("opencode-inline-shim not found; run pde install ai-tools", vim.log.levels.WARN)
    end
    if opts.on_ready then
      opts.on_ready(false)
    end
    return
  end

  local function launch()
    cleanup_stale_inline_shim({ wait_for_release = true })
    shim_job = vim.fn.jobstart({ bin }, { detach = true })
    if shim_job <= 0 then
      shim_job = nil
      vim.notify("failed to start opencode-inline-shim", vim.log.levels.ERROR)
      if opts.on_ready then
        opts.on_ready(false)
      end
      return
    end

    local shim_pid = vim.fn.jobpid(shim_job)
    if shim_pid and shim_pid > 0 then
      write_shim_pidfile(shim_pid)
    else
      clear_shim_pidfile()
    end

    wait_for_shim_ready(10, function(ready)
      if not ready then
        vim.notify("opencode-inline-shim did not become ready", vim.log.levels.ERROR)
      end
      if opts.on_ready then
        opts.on_ready(ready)
      end
    end)
  end

  if tracked_shim_running() then
    shim_healthcheck(function(ready)
      if ready then
        if opts.on_ready then
          opts.on_ready(true)
        end
        return
      end

      stop_inline_shim()
      launch()
    end)
    return
  end

  shim_healthcheck(function(ready)
    if ready then
      if opts.on_ready then
        opts.on_ready(true)
      end
      return
    end

    cleanup_stale_inline_shim({ wait_for_release = true })
    launch()
  end)
end

local function restart_inline_shim(opts)
  opts = opts or {}
  stop_inline_shim()
  start_inline_shim(opts)
end

local inline_feedback = { pending = 0, frame = 1, timer = nil }

local function redraw_statusline()
  vim.schedule(function()
    vim.cmd("redrawstatus")
  end)
end

local function set_inline_status(text)
  vim.g.pde_inline_status = text
  redraw_statusline()
end

local function clear_inline_status()
  vim.g.pde_inline_status = nil
  redraw_statusline()
end

local function is_inline_request(args)
  local data = args and args.data
  if type(data) ~= "table" then
    return false
  end

  return data.kind == "inline" or data.type == "inline" or data.interaction == "inline"
end

local function start_inline_feedback()
  inline_feedback.pending = inline_feedback.pending + 1
  if inline_feedback.pending > 1 then
    return
  end

  inline_feedback.frame = 1
  set_inline_status(string.format("%s Inline", spinner_frames[inline_feedback.frame]))

  inline_feedback.timer = vim.uv.new_timer()
  inline_feedback.timer:start(120, 120, function()
    inline_feedback.frame = (inline_feedback.frame % #spinner_frames) + 1
    set_inline_status(string.format("%s Inline", spinner_frames[inline_feedback.frame]))
  end)

  vim.notify("Generating inline edit...", vim.log.levels.INFO)
end

local function stop_inline_feedback(notify_failure)
  if inline_feedback.pending > 0 then
    inline_feedback.pending = inline_feedback.pending - 1
  end

  if notify_failure then
    vim.notify("Inline edit failed", vim.log.levels.WARN)
  end

  if inline_feedback.pending > 0 then
    return
  end

  if inline_feedback.timer then
    inline_feedback.timer:stop()
    inline_feedback.timer:close()
    inline_feedback.timer = nil
  end
  clear_inline_status()
end

local codecompanion_inline_events = vim.api.nvim_create_augroup("PDECodeCompanionInlineFeedback", { clear = true })

vim.api.nvim_create_autocmd("User", {
  group = codecompanion_inline_events,
  pattern = "CodeCompanionRequestStarted",
  callback = function(args)
    if is_inline_request(args) then
      start_inline_feedback()
    end
  end,
})

vim.api.nvim_create_autocmd("User", {
  group = codecompanion_inline_events,
  pattern = "CodeCompanionRequestFinished",
  callback = function(args)
    if not is_inline_request(args) then
      return
    end

    local status = vim.tbl_get(args, "data", "status")
    if status == "success" then
      stop_inline_feedback(false)
    else
      stop_inline_feedback(true)
    end
  end,
})

require("codecompanion").setup({
  adapters = {
    http = {
      opencode_inline = function()
        return require("codecompanion.adapters").extend("openai_compatible", {
          env = {
            api_key = "EMPTY",
            url = function()
              return shim_url()
            end,
            chat_url = "/v1/chat/completions",
            models_endpoint = "/v1/models",
          },
          schema = {
            model = {
              default = function()
                return inline_request_model()
              end,
              choices = function()
                return { inline_request_model() }
              end,
            },
          },
        })
      end,
    },
  },
  display = {
    chat = {
      window = {
        layout = "vertical",
        position = "right",
        full_height = true,
        width = 0.40,
        opts = {
          breakindent = true,
          linebreak = true,
          wrap = true,
        },
      },
    },
  },
  interactions = {
    chat = {
      adapter = "opencode",
      opts = {
        completion_provider = "blink",
      },
    },
    inline = {
      adapter = "opencode_inline",
    },
  },
})

vim.api.nvim_create_user_command("CodeCompanionOpenCodeInlineShim", function()
  restart_inline_shim({ notify_missing = true })
end, {})

local map = vim.keymap.set
local codecompanion = require("codecompanion")

local function prompt_inline()
  if not shim_bin() then
    vim.notify("opencode-inline-shim not found; run pde install ai-tools", vim.log.levels.WARN)
    return
  end

  local mode = vim.api.nvim_get_mode().mode
  local opts = {}
  if mode:find("[vV\22]") then
    opts.range = 1
  end

  start_inline_shim()

  return vim.ui.input({ prompt = require("codecompanion.config").display.action_palette.prompt }, function(input)
    if #vim.trim(input or "") == 0 then
      return
    end
    opts.args = input
    codecompanion.inline(opts)
  end)
end

local chat_keymaps = require("codecompanion.interactions.chat.keymaps")
local change_adapter = require("codecompanion.interactions.chat.keymaps.change_adapter")
local chat_helpers = require("codecompanion.interactions.chat.helpers")
local codecompanion_config = require("codecompanion.config")
local editor_buffer = require("codecompanion.interactions.shared.editor_context.buffer")
local editor_diagnostics = require("codecompanion.interactions.shared.editor_context.diagnostics")
local editor_diff = require("codecompanion.interactions.shared.editor_context.diff")
local file_slash_command = require("codecompanion.interactions.chat.slash_commands.builtin.file")
local slash_commands = require("codecompanion.interactions.chat.slash_commands")

local function current_chat()
  return codecompanion.buf_get_chat(0) or codecompanion.last_chat()
end

local function is_opencode_chat(chat)
  return chat and chat.adapter and chat.adapter.name == "opencode"
end

local function ensure_chat()
  local chat = current_chat()
  if chat then
    return chat
  end

  chat = codecompanion.chat()
  if chat and chat.ui then
    chat.ui:open()
  end
  return chat
end

local function with_chat(callback)
  local chat = current_chat()
  if not chat then
    return vim.notify("Open a CodeCompanion chat first", vim.log.levels.WARN)
  end

  return callback(chat)
end

local function inline_default_model(chat)
  local configured = configured_inline_model()
  if configured then
    return configured
  end

  local models = chat.acp_connection and chat.acp_connection:get_models()
  if models then
    return models.currentModelId
  end
  return nil
end

local function ensure_inline_model_chat(callback)
  inline_model_chat = inline_model_chat or codecompanion.chat({
    params = { adapter = "opencode" },
    auto_submit = false,
  })
  if not inline_model_chat then
    vim.notify("Failed to initialize OpenCode inline selector", vim.log.levels.ERROR)
    return
  end

  if inline_model_chat.acp_connection and inline_model_chat.acp_connection:is_ready() then
    return callback(inline_model_chat)
  end

  chat_helpers.create_acp_connection(inline_model_chat, function()
    if inline_model_chat and inline_model_chat.acp_connection and inline_model_chat.acp_connection:is_ready() then
      callback(inline_model_chat)
      return
    end
    vim.notify("OpenCode inline selector is not ready", vim.log.levels.ERROR)
  end)
end

local function inline_selector_target(chat)
  return {
    adapter = { type = "acp" },
    acp_connection = {
      get_models = function()
        local models = chat.acp_connection and chat.acp_connection:get_models()
        if not models then
          return nil
        end
        models = vim.deepcopy(models)
        models.currentModelId = vim.g.pde_inline_model or inline_default_model(chat) or models.currentModelId
        return models
      end,
    },
    change_model = function(self, args)
      local model = args and args.model
      if not model or vim.trim(model) == "" then
        return
      end
      local default_model = inline_default_model(chat)
      if default_model and model == default_model then
        vim.g.pde_inline_model = nil
      else
        vim.g.pde_inline_model = model
      end
      vim.notify("Inline model: " .. (vim.g.pde_inline_model or default_model or model), vim.log.levels.INFO)
    end,
  }
end

local function select_inline_model()
  ensure_inline_model_chat(function(chat)
    change_adapter.select_model(inline_selector_target(chat))
  end)
end

local function send_chat()
  local chat = current_chat()
  if not chat then
    return ensure_chat()
  end

  chat:submit()
  return chat
end

map("n", "<leader>pc", "<cmd>CodeCompanionChat Toggle<cr>", { desc = "Toggle chat" })
map("n", "<leader>pn", "<cmd>CodeCompanionChat<cr>", { desc = "New chat" })
map({ "n", "x" }, "<leader>pp", "<cmd>CodeCompanionActions<cr>", { desc = "Actions" })
map("n", "<leader>ps", send_chat, { desc = "Send" })
map("x", "<leader>ps", "<cmd>CodeCompanionChat Add<cr>", { desc = "Add selection" })
map("n", "<leader>pk", function()
  with_chat(function(chat)
    chat:stop()
  end)
end, { desc = "Stop" })
map({ "n", "x" }, "<leader>pe", function()
  local mode = vim.api.nvim_get_mode().mode
  if mode:find("[vV\22]") then
    return codecompanion.prompt("explain", { range = 1 })
  end

  return codecompanion.prompt("explain")
end, { desc = "Explain" })
map("n", "<leader>po", function()
  with_chat(function(chat)
    chat_keymaps.options.callback(chat)
  end)
end, { desc = "Options" })
map("n", "<leader>pm", function()
  with_chat(function(chat)
    change_adapter.select_model(chat)
  end)
end, { desc = "Select model" })
map("n", "<leader>pM", select_inline_model, { desc = "Select inline model" })
map({ "n", "x" }, "<leader>pi", prompt_inline, { desc = "Inline prompt" })
map("n", "<leader>pI", "<cmd>CodeCompanionOpenCodeInlineShim<cr>", { desc = "Restart inline shim" })
map("n", "<leader>pab", function()
  local chat = ensure_chat()
  if not chat then
    return
  end

  editor_buffer.new({ Chat = chat }):chat_render()
  chat.ui:open()
end, { desc = "Add buffer" })
map("n", "<leader>paf", function()
  local chat = ensure_chat()
  if not chat then
    return
  end

  file_slash_command.new({ Chat = chat, config = codecompanion_config }):chat_render(slash_commands.new())
  chat.ui:open()
end, { desc = "Add file" })
map("n", "<leader>pad", function()
  local chat = ensure_chat()
  if not chat then
    return
  end

  editor_diagnostics.new({ Chat = chat }):chat_render()
  chat.ui:open()
end, { desc = "Add diagnostics" })
map("n", "<leader>pag", function()
  local chat = ensure_chat()
  if not chat then
    return
  end

  editor_diff.new({ Chat = chat }):chat_render()
  chat.ui:open()
end, { desc = "Add git diff" })

-- Inline selection is intentionally coupled to OpenCode. We reuse the ACP
-- model list through change_adapter.select_model(), but inline execution
-- still stays on the local HTTP shim because CodeCompanion inline is not ACP.
