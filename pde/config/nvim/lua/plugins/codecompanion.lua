local shim_job
local shim_port = vim.env.OPENCODE_INLINE_SHIM_PORT or "4141"
local shim_url = "http://127.0.0.1:" .. shim_port

local function shim_bin()
  local bin = vim.fn.exepath("opencode-inline-shim")
  if bin == "" then
    return nil
  end
  return bin
end

local function shim_healthcheck(callback)
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

local function start_inline_shim()
  local bin = shim_bin()
  if not bin then
    vim.notify("opencode-inline-shim not found; run pde install ai-tools", vim.log.levels.WARN)
    return
  end
  if shim_job and vim.fn.jobwait({ shim_job }, 0)[1] == -1 then
    return
  end
  shim_job = vim.fn.jobstart({ bin }, { detach = true })
  if shim_job <= 0 then
    vim.notify("failed to start opencode-inline-shim", vim.log.levels.ERROR)
    return
  end
  shim_healthcheck(function(ready)
    if not ready then
      vim.notify("opencode-inline-shim did not become ready", vim.log.levels.ERROR)
    end
  end)
end

local function ensure_inline_shim()
  shim_healthcheck(function(ready)
    if not ready then
      start_inline_shim()
    end
  end)
end

require("codecompanion").setup({
  adapters = {
    http = {
      opencode_inline = function()
        return require("codecompanion.adapters").extend("openai_compatible", {
          env = {
            api_key = "EMPTY",
            url = shim_url,
            chat_url = "/v1/chat/completions",
            models_endpoint = "/v1/models",
          },
          schema = {
            model = {
              default = "opencode-inline",
              choices = { "opencode-inline" },
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

vim.api.nvim_create_user_command("CodeCompanionOpenCodeInlineShim", start_inline_shim, {})
ensure_inline_shim()

local map = vim.keymap.set
local codecompanion = require("codecompanion")
local chat_keymaps = require("codecompanion.interactions.chat.keymaps")
local change_adapter = require("codecompanion.interactions.chat.keymaps.change_adapter")
local codecompanion_config = require("codecompanion.config")
local editor_buffer = require("codecompanion.interactions.shared.editor_context.buffer")
local editor_diagnostics = require("codecompanion.interactions.shared.editor_context.diagnostics")
local editor_diff = require("codecompanion.interactions.shared.editor_context.diff")
local file_slash_command = require("codecompanion.interactions.chat.slash_commands.builtin.file")
local slash_commands = require("codecompanion.interactions.chat.slash_commands")

local function current_chat()
  return codecompanion.buf_get_chat(0) or codecompanion.last_chat()
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
map("n", "<leader>pi", "<cmd>CodeCompanionOpenCodeInlineShim<cr>", { desc = "Start inline shim" })
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

-- Inline must stay on an HTTP adapter. ACP Codex, if later added for chat,
-- should remain chat-only rather than being reused for inline.
-- Inline stays on a local HTTP adapter because CodeCompanion inline does not
-- support ACP adapters directly. The shim keeps OpenCode local to PDE.
