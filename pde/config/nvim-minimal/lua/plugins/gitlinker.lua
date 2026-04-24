require("gitlinker").setup()

local map = vim.keymap.set
map({ "n", "v" }, "<leader>gb", "<cmd>GitLink<cr>", { desc = "Git browse (copy link)" })
map({ "n", "v" }, "<leader>gB", "<cmd>GitLink!<cr>", { desc = "Git browse (open in browser)" })
