require("diffview").setup({
  enhanced_diff_hl = true,
  view = {
    default = { layout = "diff2_horizontal" },
    merge_tool = { layout = "diff3_horizontal" },
  },
})

local map = vim.keymap.set
map("n", "<leader>gv", "<cmd>DiffviewOpen<cr>",               { desc = "Diffview open" })
map("n", "<leader>gV", "<cmd>DiffviewClose<cr>",              { desc = "Diffview close" })
map("n", "<leader>gH", "<cmd>DiffviewFileHistory<cr>",        { desc = "Diffview repo history" })
map("n", "<leader>gF", "<cmd>DiffviewFileHistory %<cr>",      { desc = "Diffview file history" })
