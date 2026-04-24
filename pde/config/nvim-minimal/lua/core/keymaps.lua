local map = vim.keymap.set

-- window navigation
map("n", "<C-h>", "<C-w>h")
map("n", "<C-j>", "<C-w>j")
map("n", "<C-k>", "<C-w>k")
map("n", "<C-l>", "<C-w>l")

-- bufferline
map("n", "<S-h>", "<cmd>BufferLineCyclePrev<cr>")
map("n", "<S-l>", "<cmd>BufferLineCycleNext<cr>")
map("n", "<leader>bd", "<cmd>bdelete<cr>", { desc = "Delete buffer" })

map("n", "<leader><leader>", "<cmd>FzfLua files<cr>", { desc = "Find files" })

-- fzf-lua
map("n", "<leader>ff", "<cmd>FzfLua files<cr>", { desc = "Find files" })
map("n", "<leader>fg", "<cmd>FzfLua live_grep<cr>", { desc = "Live grep" })
map("n", "<leader>fb", "<cmd>FzfLua buffers<cr>", { desc = "Buffers" })
map("n", "<leader>fr", "<cmd>FzfLua oldfiles<cr>", { desc = "Recent files" })
map("n", "<leader>/", "<cmd>FzfLua blines<cr>", { desc = "Search buffer" })

-- terminal
map("t", "<C-q>", "<C-\\><C-n>", { desc = "Exit terminal mode" })
map("n", "<C-/>", "<cmd>split | terminal<cr><cmd>startinsert<cr>", { desc = "Open terminal" })
map("n", "<C-_>", "<cmd>split | terminal<cr><cmd>startinsert<cr>", { desc = "Open terminal" })

-- misc
map("n", "<Esc>", "<cmd>nohlsearch<cr>")
