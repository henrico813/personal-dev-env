local function lazygit(cwd)
  local buf = vim.api.nvim_create_buf(false, true)
  local width = math.floor(vim.o.columns * 0.9)
  local height = math.floor(vim.o.lines * 0.9)
  local win = vim.api.nvim_open_win(buf, true, {
    relative = "editor",
    width = width,
    height = height,
    col = math.floor((vim.o.columns - width) / 2),
    row = math.floor((vim.o.lines - height) / 2),
    style = "minimal",
    border = "rounded",
  })
  local cmd = cwd and ("lazygit -p " .. cwd) or "lazygit"
  vim.fn.termopen(cmd, {
    on_exit = function()
      vim.api.nvim_win_close(win, true)
    end,
  })
  vim.cmd("startinsert")
end

local map = vim.keymap.set

-- lazygit (only map if installed)
if vim.fn.executable("lazygit") == 1 then
  map("n", "<leader>gg", function() lazygit() end, { desc = "Lazygit (root)" })
  map("n", "<leader>gG", function() lazygit(vim.fn.getcwd()) end, { desc = "Lazygit (cwd)" })
end

-- git log/history via fzf-lua
map("n", "<leader>gl", "<cmd>FzfLua git_commits<cr>", { desc = "Git log" })
map("n", "<leader>gL", "<cmd>FzfLua git_commits cwd=" .. vim.fn.getcwd() .. "<cr>", { desc = "Git log (cwd)" })
map("n", "<leader>gf", "<cmd>FzfLua git_bcommits<cr>", { desc = "Git file history" })
