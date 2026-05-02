require("codecompanion").setup({
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
  },
})

local map = vim.keymap.set

map("n", "<leader>pc", "<cmd>CodeCompanionChat Toggle<cr>", { desc = "Toggle chat" })
map("n", "<leader>pn", "<cmd>CodeCompanionChat<cr>", { desc = "New chat" })
map({ "n", "x" }, "<leader>pp", "<cmd>CodeCompanionActions<cr>", { desc = "Actions" })
map("x", "<leader>pa", "<cmd>CodeCompanionChat Add<cr>", { desc = "Add selection to chat" })
