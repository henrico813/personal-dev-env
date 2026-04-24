local map = vim.keymap.set

-- window navigation
map("n", "<C-h>", "<C-w>h")
map("n", "<C-j>", "<C-w>j")
map("n", "<C-k>", "<C-w>k")
map("n", "<C-l>", "<C-w>l")

-- buffers
map("n", "<S-h>",       "<cmd>BufferLineCyclePrev<cr>",  { desc = "Prev buffer" })
map("n", "<S-l>",       "<cmd>BufferLineCycleNext<cr>",  { desc = "Next buffer" })
map("n", "<leader>bb",  "<cmd>FzfLua buffers<cr>",       { desc = "Pick buffer" })
map("n", "<leader>bd",  "<cmd>bdelete<cr>",              { desc = "Delete buffer" })
map("n", "<leader>bD",  "<cmd>bdelete!<cr>",             { desc = "Delete buffer (force)" })
map("n", "<leader>bo",  "<cmd>%bd|e#|bd#<cr>",           { desc = "Delete other buffers" })
map("n", "<leader>bn",  "<cmd>bnext<cr>",                { desc = "Next buffer" })
map("n", "<leader>bp",  "<cmd>bprevious<cr>",            { desc = "Prev buffer" })
map("n", "<leader>bw",  "<cmd>write<cr>",                { desc = "Write buffer" })
map("n", "<leader>bW",  "<cmd>wall<cr>",                 { desc = "Write all buffers" })
map("n", "<leader>bP",  "<cmd>BufferLineTogglePin<cr>",  { desc = "Pin buffer" })

map("n", "<leader><leader>", "<cmd>FzfLua files<cr>", { desc = "Find files" })

-- fzf-lua
map("n", "<leader>ff", "<cmd>FzfLua files<cr>", { desc = "Find files" })
map("n", "<leader>fg", "<cmd>FzfLua live_grep<cr>", { desc = "Live grep" })
map("n", "<leader>fb", "<cmd>FzfLua buffers<cr>", { desc = "Buffers" })
map("n", "<leader>fr", "<cmd>FzfLua oldfiles<cr>", { desc = "Recent files" })
map("n", "<leader>/", "<cmd>FzfLua blines<cr>", { desc = "Search buffer" })

-- tabs
map("n", "<leader><Tab><Tab>", "<cmd>tabnew<cr>",       { desc = "New tab" })
map("n", "<leader><Tab>d",     "<cmd>tabclose<cr>",     { desc = "Close tab" })
map("n", "<leader><Tab>o",     "<cmd>tabonly<cr>",      { desc = "Close other tabs" })
map("n", "<leader><Tab>l",     "<cmd>tabnext<cr>",      { desc = "Next tab" })
map("n", "<leader><Tab>h",     "<cmd>tabprevious<cr>",  { desc = "Prev tab" })
map("n", "<leader><Tab>f",     "<cmd>tabfirst<cr>",     { desc = "First tab" })
map("n", "<leader><Tab>L",     "<cmd>tablast<cr>",      { desc = "Last tab" })
map("n", "<leader><Tab>T",     "<C-w>T",                { desc = "Move window to new tab" })

-- terminal
map("t", "<C-q>", "<C-\\><C-n>", { desc = "Exit terminal mode" })
map("n", "<C-/>", "<cmd>split | terminal<cr><cmd>startinsert<cr>", { desc = "Open terminal" })
map("n", "<C-_>", "<cmd>split | terminal<cr><cmd>startinsert<cr>", { desc = "Open terminal" })

-- misc
map("n", "<Esc>", "<cmd>nohlsearch<cr>")
