require("pi").setup({
  layout = {
    default = "side",
    side = { position = "right", width = 80 },
  },
  diff = {
    keys = {
      accept         = "<Leader>da",
      reject         = "<Leader>dr",
      expand_context = "<Leader>de",
      shrink_context = "<Leader>ds",
    },
  },
})

local map = vim.keymap.set

map("n", "<leader>pc", "<cmd>Pi<cr>",                { desc = "Toggle chat" })
map("n", "<leader>pC", "<cmd>PiContinue<cr>",        { desc = "Continue last session" })
map("n", "<leader>pw", "<cmd>PiResume<cr>",          { desc = "Resume session (picker)" })
map("n", "<leader>pn", "<cmd>PiNewSession<cr>",      { desc = "New session" })
map("n", "<leader>pa", "<cmd>PiAbort<cr>",           { desc = "Abort turn" })
map("n", "<leader>pS", "<cmd>PiStop<cr>",            { desc = "Stop agent" })
map("n", "<leader>pm", "<cmd>PiCycleModel<cr>",      { desc = "Cycle model" })
map("n", "<leader>pM", "<cmd>PiSelectModel<cr>",     { desc = "Select model" })
map("n", "<leader>pt", "<cmd>PiCycleThinking<cr>",   { desc = "Cycle thinking level" })
map("n", "<leader>pT", "<cmd>PiSelectThinking<cr>",  { desc = "Select thinking level" })
map("n", "<leader>p@", "<cmd>PiSendMention<cr>",     { desc = "Send @mention" })
map("n", "<leader>pK", "<cmd>PiCompact<cr>",         { desc = "Compact context" })
map("n", "<leader>pN", "<cmd>PiSessionName<cr>",     { desc = "Set session name" })
map("n", "<leader>pd", "<cmd>PiToggleDebug<cr>",     { desc = "Toggle debug logging" })

-- Auto-continue last pi session after persistence restore
vim.api.nvim_create_autocmd("User", {
  pattern = "PersistenceLoadPost",
  callback = function()
    vim.schedule(function()
      pcall(vim.cmd, "PiContinue")
    end)
  end,
})
