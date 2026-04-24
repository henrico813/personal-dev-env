require("pi").setup({
  auto_connect = false,
  approval_mode = true,
  keymaps = {
    toggle_panel = "<leader>pt",
    toggle_logs  = "<leader>pl",
    toggle_chat  = "<leader>pc",
    approve      = "<leader>pa",
    reject       = "<leader>pr",
  },
})
