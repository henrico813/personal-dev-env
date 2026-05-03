require("codecompanion").setup({
  adapters = {
    http = {
      openai_responses = function()
        return require("codecompanion.adapters").extend("openai_responses", {
          env = {
            api_key = "OPENAI_API_KEY",
          },
          schema = {
            model = {
              default = "gpt-5.3-codex",
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
      adapter = "openai_responses",
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
  if vim.fn.mode():match("^[vV\22]") then
    return codecompanion.prompt("explain", { range = 1 })
  end

  local chat = ensure_chat()
  if not chat then
    return
  end

  editor_buffer.new({ Chat = chat }):chat_render()
  chat:add_message({
    role = codecompanion_config.constants.USER_ROLE,
    content = "Please explain this code.",
  })
  chat.ui:open()
  chat:submit()
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
map("n", "<leader>p?", "<cmd>help codecompanion-welcome<cr>", { desc = "CodeCompanion help" })
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
