local pde_paths = require("core.pde_paths")
local opencode_inline = require("opencode-inline")

local opencode_keys = { "OPENCODE_BASE_URL", "OPENCODE_INLINE_SHIM_PORT", "OPENCODE_INLINE_MODEL" }
local opencode_config = pde_paths.read(opencode_keys)

-- Environment inherited by Neovim overrides ~/.config/pde/config.json.
local function opencode_setting(key)
  for _, value in ipairs({ vim.env[key], opencode_config[key] }) do
    if type(value) == "string" and vim.trim(value) ~= "" then
      return value
    end
  end
  return nil
end

opencode_inline.setup({
  opencode_url = opencode_setting("OPENCODE_BASE_URL"),
  port = opencode_setting("OPENCODE_INLINE_SHIM_PORT"),
  model = opencode_setting("OPENCODE_INLINE_MODEL"),
})

require("codecompanion").setup({
  adapters = {
    http = {
      opencode_inline = opencode_inline.adapter,
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
map("n", "<leader>pM", opencode_inline.select_model, { desc = "Select inline model" })
map({ "n", "x" }, "<leader>pi", opencode_inline.prompt, { desc = "Inline prompt" })
map("n", "<leader>pI", "<cmd>OpenCodeInlineRestart<cr>", { desc = "Restart inline shim" })
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
