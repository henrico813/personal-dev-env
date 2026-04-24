local opt = vim.opt

opt.number = true
opt.relativenumber = true
opt.expandtab = true
opt.shiftwidth = 2
opt.tabstop = 2
opt.termguicolors = true
opt.signcolumn = "yes"
opt.scrolloff = 8
opt.splitright = true
opt.splitbelow = true
opt.wrap = false
opt.ignorecase = true
opt.smartcase = true
opt.clipboard = "unnamedplus"
opt.undofile = true
opt.updatetime = 200
opt.cursorline = true
opt.linebreak = true
opt.mouse = "a"
opt.fillchars = { vert = "│", eob = " " }
opt.winbar = "%=%t %m"

vim.api.nvim_set_hl(0, "VertSplit",   { fg = "#7aa2f7", bg = "NONE" })
vim.api.nvim_set_hl(0, "PiNormal",        { bg = "#16161e" })
vim.api.nvim_set_hl(0, "PiChatBar",      { fg = "#7aa2f7", bg = "#16161e", bold = true })
vim.api.nvim_set_hl(0, "PiChatBarNC",    { fg = "#3b4261", bg = "#16161e" })
vim.api.nvim_set_hl(0, "PiInputBar",     { fg = "#9ece6a", bg = "#16161e", bold = true })
vim.api.nvim_set_hl(0, "PiInputBarNC",   { fg = "#3b4261", bg = "#16161e" })
vim.api.nvim_set_hl(0, "PiStatusBar",    { fg = "#c0caf5", bg = "#16161e", bold = true })
vim.api.nvim_set_hl(0, "PiStatusBarNC",  { fg = "#3b4261", bg = "#16161e" })

local function clamp_pi_windows()
  local max_width = math.floor(vim.o.columns * 0.40)
  local min_width = math.floor(vim.o.columns * 0.25)
  for _, win in ipairs(vim.api.nvim_list_wins()) do
    local buf = vim.api.nvim_win_get_buf(win)
    local name = vim.fn.fnamemodify(vim.api.nvim_buf_get_name(buf), ":t")
    if name:match("^Pi") then
      local width = vim.api.nvim_win_get_width(win)
      if width > max_width then
        vim.api.nvim_win_set_width(win, max_width)
      elseif width < min_width then
        vim.api.nvim_win_set_width(win, min_width)
      end
    end
  end
end

vim.api.nvim_create_autocmd({ "WinResized", "VimResized" }, {
  callback = clamp_pi_windows,
})

vim.api.nvim_create_autocmd("BufWinEnter", {
  callback = function()
    local name = vim.api.nvim_buf_get_name(0)
    local tail = vim.fn.fnamemodify(name, ":t")
    if tail:match("^Pi") then
      vim.wo.number = false
      vim.wo.relativenumber = false
      vim.wo.linebreak = true
    end
    if tail == "PiChatInput" then
      vim.wo.winhighlight = "Normal:PiNormal,WinBar:PiInputBar,WinBarNC:PiInputBarNC"
    elseif tail == "PiChatStatus" then
      vim.wo.winhighlight = "Normal:PiNormal,WinBar:PiStatusBar,WinBarNC:PiStatusBarNC"
    elseif tail:match("^Pi") then
      vim.wo.winhighlight = "Normal:PiNormal,WinBar:PiChatBar,WinBarNC:PiChatBarNC"
    end
  end,
})
vim.api.nvim_set_hl(0, "NormalNC",   { bg = "#0c0e14" })
vim.api.nvim_set_hl(0, "WinBar",     { fg = "#7dcfff", bg = "#292e42", bold = true })
vim.api.nvim_set_hl(0, "WinBarNC",   { fg = "#3b4261", bg = "#0c0e14" })
